//! Read-only conversation projection of the existing board and execution ledger.
use crate::{domain::*, store::Store};
use rusqlite::params;
use serde_json::{Value, json};

impl Store {
    pub fn chat_events(
        &self,
        root: Option<&str>,
        after: Option<i64>,
        before: Option<i64>,
    ) -> PResult<Value> {
        let db = self.lock()?;
        let mut stmt = db.prepare(
            "WITH member_roots AS (
                SELECT instance_id,root_id FROM board_agents UNION SELECT instance_id,root_id FROM board_peer_roots
            ), run_links AS (
                SELECT run_id,root_id,task_id,instance_id FROM board_agents
                UNION
                SELECT i.run_id,mr.root_id,NULL,r.instance_id FROM message_inbox i
                JOIN runs r ON r.id=i.run_id JOIN messages m ON m.id=i.message_id
                JOIN member_roots mr ON mr.instance_id=m.sender_instance_id
            ), run_scope AS (
                SELECT run_id,root_id,max(task_id) AS task_id,instance_id
                FROM run_links GROUP BY run_id,root_id,instance_id
            ), projected AS (
            SELECT DISTINCT e.seq,e.kind,e.subject,e.body,e.at,
                coalesce(t.root_id,s.root_id) AS root_id,coalesce(t.id,s.task_id) AS task_id,
                coalesce(s.instance_id,json_extract(e.body,'$.instance_id')) AS instance_id,
                r.id AS run_id,coalesce(a.preset_name,ta.preset_name) AS preset_name
            FROM events e
            LEFT JOIN board_tasks t ON t.id=e.subject
            LEFT JOIN run_scope s ON s.run_id=e.subject
            LEFT JOIN runs r ON r.id=s.run_id
            LEFT JOIN instances a ON a.id=s.instance_id
            LEFT JOIN instances ta ON ta.id=json_extract(e.body,'$.instance_id')
            WHERE coalesce(t.root_id,s.root_id) IS NOT NULL
                AND (?1 IS NULL OR coalesce(t.root_id,s.root_id)=?1)
                AND (?2 IS NULL OR e.seq>?2) AND (?3 IS NULL OR e.seq<?3)
                AND e.kind IN ('board.task_published','board.task_claimed','board.task_split',
                    'board.peer_recruited','board.progress_updated','board.task_settled',
                    'board.task_reviewed','board.audit_settled','board.routing_deferred',
                    'execution.started','execution.model_started','execution.model_completed',
                    'execution.tool_started','execution.tool_completed','execution.finished',
                    'execution.text_delta','execution.text_end')
            ), ranked AS (
                SELECT *,dense_rank() OVER (ORDER BY CASE WHEN ?2 IS NULL THEN -seq ELSE seq END) AS event_rank
                FROM projected
            )
            SELECT * FROM ranked WHERE event_rank<=201
            ORDER BY event_rank,root_id"
        )?;
        let mut events = stmt.query_map(params![root,after,before], |r| {
            Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?))
        })?.map(|row| {
            let (seq,kind,subject,body,at,root,task,instance,run,preset) = row?;
            let body: Value = serde_json::from_str(&body)?;
            let data = match kind.as_str() {
                "execution.text_delta" | "execution.text_end" => body,
                "execution.model_completed" => json!({"text":body["data"]["text"],"tools":body["data"]["tools"]}),
                "execution.tool_started" => json!({"name":body["data"]["name"]}),
                "execution.tool_completed" => json!({"name":body["data"]["call"]["name"],"changed":body["data"]["output"]["changed"],"error":body["data"]["output"]["content"]["error"]}),
                "execution.finished" => json!({"error":body["data"]["error"]}),
                "execution.started" | "execution.model_started" => json!({}),
                _ => body,
            };
            Ok(json!({"seq":seq,"kind":kind,"subject":subject,"at":at,"root_id":root,"task_id":task,"instance_id":instance,"run_id":run,"preset_name":preset,"data":data}))
        }).collect::<PResult<Vec<_>>>()?;
        // A run may contribute to several roots. Keep every projection of the
        // boundary event together so a seq-only cursor cannot skip a root.
        let mut sequences = events
            .iter()
            .filter_map(|e| e["seq"].as_i64())
            .collect::<Vec<_>>();
        sequences.dedup();
        let has_more = sequences.len() > 200;
        if let Some(boundary) = sequences.get(200) {
            events.retain(|e| e["seq"].as_i64() != Some(*boundary));
        }
        if after.is_none() {
            events.reverse();
        }
        let cursor: i64 = if after.is_some() {
            events
                .last()
                .and_then(|e| e["seq"].as_i64())
                .unwrap_or(
                    db.query_row("SELECT coalesce(max(seq),0) FROM events", [], |r| r.get(0))?,
                )
        } else {
            db.query_row("SELECT coalesce(max(seq),0) FROM events", [], |r| r.get(0))?
        };
        Ok(json!({"events":events,"cursor":cursor,"has_more":has_more}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_keeps_all_roots_of_an_event_together() {
        let store = Store::memory().unwrap();
        store.lock().unwrap().execute_batch(
            "INSERT INTO presets VALUES('fixture','1','{}',0);
             INSERT INTO instances VALUES('sender','fleet','fixture','1','idle','{}',0);
             INSERT INTO instances VALUES('recipient','fleet','fixture','1','idle','{}',0);
             INSERT INTO runs(id,instance_id,state,input,created_at) VALUES('source','sender','completed','',0),('shared','recipient','running','',0);
             INSERT INTO board_tasks(id,root_id,title,instructions,acceptance,state,depth,created_at)
                VALUES('root-a','root-a','a','a','a','pending',0,0),('root-b','root-b','b','b','b','pending',0,0);
             INSERT INTO board_agents(instance_id,root_id,task_id,run_id,created_at) VALUES('sender','root-a','root-a','source',0);
             INSERT INTO messages(id,sender_kind,sender_instance_id,source_run_id,recipient_id,text,correlation_id,depth,created_at)
                VALUES('handoff','agent','sender','source','recipient','work','source',0,0);
             INSERT INTO board_peer_roots VALUES('sender','root-b','handoff',0);
             INSERT INTO message_inbox(message_id,recipient_id,state,run_id,received_at) VALUES('handoff','recipient','running','shared',0);"
        ).unwrap();
        for n in 0..215 {
            store
                .record(
                    "execution.text_delta",
                    "shared",
                    json!({"generation":"g","offset":n,"text":"x"}),
                )
                .unwrap();
        }
        let latest = store.chat_events(None, None, None).unwrap();
        assert_eq!(latest["has_more"], true);
        assert_eq!(latest["events"].as_array().unwrap().len(), 400);
        let boundary = latest["events"][0]["seq"].as_i64().unwrap();
        let older = store.chat_events(None, None, Some(boundary)).unwrap();
        assert_eq!(older["events"].as_array().unwrap().len(), 30);
        let live = store.chat_events(None, Some(0), None).unwrap();
        assert_eq!(live["events"].as_array().unwrap().len(), 400);
        let next = store
            .chat_events(None, live["cursor"].as_i64(), None)
            .unwrap();
        assert_eq!(next["events"].as_array().unwrap().len(), 30);
        for pair in latest["events"].as_array().unwrap().as_chunks::<2>().0 {
            assert_eq!(pair[0]["seq"], pair[1]["seq"]);
            assert_ne!(pair[0]["root_id"], pair[1]["root_id"]);
        }
        store.record("unrelated", "other", json!({})).unwrap();
        let empty = store.chat_events(Some("unknown"), Some(0), None).unwrap();
        assert!(empty["events"].as_array().unwrap().is_empty());
        assert!(empty["cursor"].as_i64() > latest["cursor"].as_i64());
    }
}
