use super::*;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct Frames {
    bytes: Vec<u8>,
    data: Vec<String>,
}
impl Frames {
    fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>> {
        self.bytes.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while let Some(end) = self.bytes.iter().position(|&b| b == b'\n') {
            let line = self.bytes.drain(..=end).collect::<Vec<_>>();
            let line = std::str::from_utf8(&line)
                .map_err(|_| Error::new("provider_protocol_error"))?
                .trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if !self.data.is_empty() {
                    frames.push(self.data.join("\n"));
                    self.data.clear();
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                self.data
                    .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
            }
        }
        Ok(frames)
    }
}

struct Decoder {
    protocol: Protocol,
    body: Value,
    tools: BTreeMap<u64, Value>,
    blocks: BTreeMap<u64, (Value, String)>,
    done: bool,
}
impl Decoder {
    fn new(protocol: &Protocol) -> Self {
        Self {
            protocol: protocol.clone(),
            body: json!({"choices":[{"message":{"content":"","tool_calls":[]},"finish_reason":null}]}),
            tools: BTreeMap::new(),
            blocks: BTreeMap::new(),
            done: false,
        }
    }
    fn event(&mut self, frame: &str, live: &LiveOutput) -> Result<()> {
        if frame == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(frame).map_err(|_| Error::new("provider_protocol_error"))?;
        if v.get("error").is_some() || v["type"] == "error" {
            return Err(Error::new("provider_stream_error"));
        }
        match self.protocol {
            Protocol::OpenaiCompatible => {
                if let Some(usage) = v.get("usage").filter(|v| !v.is_null()) {
                    self.body["usage"] = usage.clone();
                }
                let Some(choices) = v["choices"].as_array() else {
                    return Err(Error::new("provider_protocol_error"));
                };
                for choice in choices
                    .iter()
                    .filter(|c| c["index"].as_u64().unwrap_or(0) == 0)
                {
                    if let Some(text) = choice["delta"]["content"].as_str() {
                        let out = self.body["choices"][0]["message"]["content"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned()
                            + text;
                        self.body["choices"][0]["message"]["content"] = json!(out);
                        live.text(text);
                    }
                    if choice["delta"]["refusal"].as_str().is_some() {
                        return Err(Error::new("provider_refusal"));
                    }
                    for call in choice["delta"]["tool_calls"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        let index = call["index"]
                            .as_u64()
                            .ok_or_else(|| Error::new("provider_protocol_error"))?;
                        let tool=self.tools.entry(index).or_insert_with(||json!({"id":"","type":"function","function":{"name":"","arguments":""}}));
                        for key in ["id", "name", "arguments"] {
                            let part = if key == "id" {
                                &call[key]
                            } else {
                                &call["function"][key]
                            };
                            if let Some(part) = part.as_str() {
                                let target = if key == "id" {
                                    &mut tool[key]
                                } else {
                                    &mut tool["function"][key]
                                };
                                *target =
                                    json!(target.as_str().unwrap_or_default().to_owned() + part);
                            }
                        }
                    }
                    if !choice["finish_reason"].is_null() {
                        self.body["choices"][0]["finish_reason"] = choice["finish_reason"].clone();
                    }
                }
            }
            Protocol::OpenaiResponses => match v["type"].as_str() {
                Some("response.output_text.delta") => live.text(
                    v["delta"]
                        .as_str()
                        .ok_or_else(|| Error::new("provider_protocol_error"))?,
                ),
                Some("response.completed") => {
                    self.body = v["response"].clone();
                    self.done = true;
                }
                Some("response.failed" | "response.incomplete") => {
                    return Err(Error::new("provider_incomplete"));
                }
                _ => {}
            },
            Protocol::Anthropic => match v["type"].as_str() {
                Some("message_start") => self.body = v["message"].clone(),
                Some("content_block_start") => {
                    let index = v["index"]
                        .as_u64()
                        .ok_or_else(|| Error::new("provider_protocol_error"))?;
                    let block = v["content_block"].clone();
                    if block["type"] == "text" {
                        live.text(block["text"].as_str().unwrap_or_default());
                    }
                    self.blocks.insert(index, (block, String::new()));
                }
                Some("content_block_delta") => {
                    let (block, args) = self
                        .blocks
                        .get_mut(
                            &v["index"]
                                .as_u64()
                                .ok_or_else(|| Error::new("provider_protocol_error"))?,
                        )
                        .ok_or_else(|| Error::new("provider_protocol_error"))?;
                    match v["delta"]["type"].as_str() {
                        Some("text_delta") => {
                            let text = v["delta"]["text"]
                                .as_str()
                                .ok_or_else(|| Error::new("provider_protocol_error"))?;
                            block["text"] =
                                json!(block["text"].as_str().unwrap_or_default().to_owned() + text);
                            live.text(text);
                        }
                        Some("input_json_delta") => args.push_str(
                            v["delta"]["partial_json"]
                                .as_str()
                                .ok_or_else(|| Error::new("provider_protocol_error"))?,
                        ),
                        _ => return Err(Error::new("unsupported_provider_output")),
                    }
                }
                Some("message_delta") => {
                    self.body["stop_reason"] = v["delta"]["stop_reason"].clone();
                    if let Some(usage) = v["usage"].as_object() {
                        for (key, value) in usage {
                            self.body["usage"][key] = value.clone();
                        }
                    }
                }
                Some("message_stop") => self.done = true,
                _ => {}
            },
            Protocol::Demo => return Err(Error::new("invalid_provider")),
        }
        Ok(())
    }
    fn finish(mut self) -> Result<ModelReply> {
        if !self.done {
            return Err(Error::new("provider_incomplete"));
        }
        match self.protocol {
            Protocol::OpenaiCompatible => {
                self.body["choices"][0]["message"]["tool_calls"] =
                    json!(self.tools.into_values().collect::<Vec<_>>())
            }
            Protocol::Anthropic => {
                let mut content = Vec::new();
                for (mut block, args) in self.blocks.into_values() {
                    if block["type"] == "tool_use" && !args.is_empty() {
                        block["input"] = serde_json::from_str(&args)
                            .map_err(|_| Error::new("invalid_tool_json"))?;
                    }
                    content.push(block);
                }
                self.body["content"] = json!(content);
            }
            _ => {}
        }
        Provider::decode(&self.protocol, self.body)
    }
}

pub(super) async fn receive(
    mut response: reqwest::Response,
    protocol: &Protocol,
    live: &LiveOutput,
    cancel: CancellationToken,
) -> Result<ModelReply> {
    let mut frames = Frames::default();
    let mut decoder = Decoder::new(protocol);
    let mut total = 0;
    while let Some(chunk) = tokio::select! { _=cancel.cancelled()=>return Err(Error::new("cancelled")), chunk=response.chunk()=>chunk.map_err(network_error)? }
    {
        total += chunk.len();
        if total > MAX_BYTES {
            return Err(Error::new("provider_response_too_large"));
        }
        for frame in frames.push(&chunk)? {
            decoder.event(&frame, live)?;
        }
        if decoder.done {
            return decoder.finish();
        }
    }
    decoder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_survive_fragmented_utf8_and_crlf() {
        let bytes = "event: message\r\ndata: {\"text\":\"蚂蚁\"}\r\n\r\n".as_bytes();
        let mut parser = Frames::default();
        let mut frames = Vec::new();
        for byte in bytes {
            frames.extend(parser.push(&[*byte]).unwrap());
        }
        assert_eq!(frames, vec!["{\"text\":\"蚂蚁\"}"]);
    }
    #[test]
    fn compatible_stream_collects_split_tools_usage_and_rejects_missing_terminator() {
        let live = LiveOutput::default();
        let mut d = Decoder::new(&Protocol::OpenaiCompatible);
        for value in [
            json!({"choices":[{"index":0,"delta":{"content":"开始","tool_calls":[{"index":0,"id":"call-1","function":{"name":"read","arguments":"{\"a\":"}}]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1}"}}]},"finish_reason":"tool_calls"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":5}}),
        ] {
            d.event(&value.to_string(), &live).unwrap();
        }
        assert_eq!(live.text_so_far(), "开始");
        d.event("[DONE]", &live).unwrap();
        let tools = d.finish().unwrap();
        assert_eq!(tools.calls[0].id, "call-1");
        assert_eq!(tools.calls[0].arguments, json!({"a":1}));
        let mut missing = Decoder::new(&Protocol::OpenaiCompatible);
        missing
            .event(
                &json!({"choices":[{"delta":{"content":"partial"},"finish_reason":"stop"}]})
                    .to_string(),
                &live,
            )
            .unwrap();
        assert_eq!(missing.finish().err().unwrap().code, "provider_incomplete");
        let mut d = Decoder::new(&Protocol::OpenaiCompatible);
        d.event(&json!({"choices":[{"delta":{"content":"完成"},"finish_reason":"stop"}],"usage":{"prompt_tokens":20,"completion_tokens":5}}).to_string(),&live).unwrap();
        d.event("[DONE]", &live).unwrap();
        let reply = d.finish().unwrap();
        assert_eq!(reply.text, "完成");
        assert_eq!(reply.usage.unwrap().input, 20);
    }
    #[test]
    fn responses_keeps_final_continuation_and_anthropic_keeps_tool_input() {
        let live = LiveOutput::default();
        let mut d = Decoder::new(&Protocol::OpenaiResponses);
        d.event(
            &json!({"type":"response.output_text.delta","delta":"结果"}).to_string(),
            &live,
        )
        .unwrap();
        d.event(&json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"结果"}]}]}}).to_string(),&live).unwrap();
        assert_eq!(
            d.finish().unwrap().continuation["output"][0]["content"][0]["text"],
            "结果"
        );
        let mut d = Decoder::new(&Protocol::Anthropic);
        for value in [
            json!({"type":"message_start","message":{"usage":{"input_tokens":12}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"c1","name":"read","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a\"}"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}),
            json!({"type":"message_stop"}),
        ] {
            d.event(&value.to_string(), &live).unwrap();
        }
        let r = d.finish().unwrap();
        assert_eq!(r.calls[0].arguments, json!({"path":"a"}));
        assert_eq!(r.usage.unwrap().output, 5);
    }
}
