'use strict';
const $ = (s) => document.querySelector(s);
const esc = (value = '') => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const short = s => String(s ?? '').slice(0, 15);
const time = value => value ? new Date(value).toLocaleString('zh-CN', {month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false}) : '未记录';
const labels = {verifying:'独立验证',ready:'就绪',draining:'排空中',busy:'执行中',suspended:'已暂停',terminated:'已终止',queued:'排队中',running:'运行中',completed:'已完成',failed:'失败',interrupted:'中断（结果未知）',cancelled:'已取消',implemented:'已实现',partial:'部分实现',planned:'规划中'};
const badge = (s, text) => `<span class="badge ${esc(s)}">${esc(text || labels[s] || s)}</span>`;
Object.assign(labels,{delegating:'委派准备中',active:'已协调',provisioning:'协调中',superseded:'已被替代',pending:'等待派发',blocked:'依赖阻断','awaiting-review':'待验收',accepted:'已验收',cancelling:'正在取消'});
const runErrorText = code => ({provider_network_error:'模型服务连接失败，请检查网关与网络配置',model_not_configured:'尚未配置模型凭证',timeout:'执行超时',cancelled:'运行已取消',interrupted_outcome_unknown:'执行结果未知，需要人工核对',empty_response:'模型返回了空响应',required_tool_not_completed:'未完成必需的工具调用',tool_limit:'工具调用次数超限',round_limit:'对话轮次超限',context_limit:'上下文超出模型窗口',invalid_arguments:'工具参数不合法',session_busy:'会话正忙'})[code]||'执行未完成';
const toolNames = {'platform.board.read':'查看任务板','platform.board.claim':'认领工作','platform.board.split':'拆分工作','platform.board.progress':'上报进展','platform.board.result':'读取同伴成果','platform.board.traces':'查看痕迹','platform.board.trace':'留下信号','platform.board.notes':'查看共享结论','platform.board.note':'发布结论','platform.board.audit.evidence':'读取待审成果','platform.board.audit.report':'提交质检结论','text.inspect':'检查文本','web.search':'搜索资料','web.fetch':'读取网页','platform.messaging.send':'发送交接','platform.messaging':'消息操作','platform.workspace.write':'写入文件','platform.workspace.read':'读取文件','platform.workspace.publish':'发布文件','platform.workspace.list':'浏览文件'};
function toolLabel(name){ return toolNames[name] || (name.startsWith('platform.')?'平台操作':name.startsWith('ant3_')?'内置工具':'自定义工具'); }
function toolSummary(item){
  const c=item.output?.content||{};
  if(c && c.error) return '未完成';
  const name=item.call.name;
  if(name==='platform.board.read') return `看到 ${Array.isArray(c.tasks)?c.tasks.length:0} 项工作`;
  if(name==='platform.board.claim') return c.title?`认领了「${c.title}」`:'认领了一项工作';
  if(name==='platform.board.split') return `拆成 ${Array.isArray(c.task_ids)?c.task_ids.length:0} 份子工作`;
  if(name==='platform.board.progress') return String(c.message||'上报了进展').slice(0,80);
  if(name==='platform.board.result') return '读完了一份同伴成果';
  if(name==='platform.board.traces') return `看到 ${Array.isArray(c.traces)?c.traces.length:0} 条痕迹`;
  if(name==='platform.board.trace') return c.key?`留下了「${c.key}」信号`:'留下了信号';
  if(name==='platform.board.notes') return `看到 ${Array.isArray(c.notes)?c.notes.length:0} 条共享结论`;
  if(name==='platform.board.note') return c.key?`发布了「${c.key}」`:'发布了结论';
  if(name==='platform.board.audit.evidence') return c.complete?'完整读取了待审成果':'读取了部分待审成果';
  if(name==='platform.board.audit.report') return '提交了质检结论';
  if(name==='text.inspect'&&Number.isFinite(Number(c.characters))) return `统计出 ${Number(c.characters).toLocaleString()} 个字符`;
  if(name==='web.search') return `找到 ${Array.isArray(c.results)?c.results.length:0} 条结果`;
  if(name==='web.fetch') return '读取了网页内容';
  if(name==='platform.messaging.send') return '发送了工作交接';
  if(name.startsWith('platform.workspace')) return '完成文件操作';
  return item.output?.changed?'完成并产生变更':'完成';
}
function durationText(ms){
  const value=Number(ms);if(!Number.isFinite(value)||value<0)return '待结算';
  if(value<1000)return `${value} 毫秒`;
  if(value<60000)return `${(value/1000).toFixed(value<10000?1:0)} 秒`;
  const minutes=Math.floor(value/60000),seconds=Math.round((value%60000)/1000);
  return `${minutes} 分 ${String(seconds).padStart(2,'0')} 秒`;
}
const eventNames = {board:{'task_published':'工作发布','task_routed':'成员安排','task_claimed':'成员接活','task_split':'工作拆分','children_activated':'子任务开工','continuation_ready':'等待汇总','task_reviewed':'评审完成','task_requeued':'重新开放','task_settled':'工作结算','progress_updated':'进展上报','trace_deposited':'痕迹沉积','audit_requested':'质检安排','audit_dispatched':'质检开始','audit_settled':'质检完成','note_published':'结论发布','note_adjudicated':'结论裁决','energy_posted':'能量记账','rework_cooldown':'暂缓重试'},swarm:{'self_selected':'个体接活','peer_spawned':'成员加入','peer_retired':'成员退出','colony_starved':'群体预算耗尽'}};
function eventLabel(kind){
  const [group, name] = String(kind).split('.');
  return eventNames[group]?.[name] || (typeof studioEventName==='function'?studioEventName(kind):'系统事件');
}
let data = null, events = [], cursor = 0, authenticated = false, modalContext = null, refreshTask = null;
let toastTimer;

// Inline icons keep the local console independent of network fonts.
const iconPaths = {
  board:'<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M8 8h8M8 12h8M8 16h4"/>',
  studio:'<circle cx="6" cy="7" r="3"/><circle cx="18" cy="7" r="3"/><circle cx="12" cy="18" r="3"/><path d="m6 10 4 6m8-6-4 6M9 7h6"/>',
  overview:'<path d="m12 3 9 5v8l-9 5-9-5V8Z"/><path d="m3 8 9 5 9-5M12 13v8M7.5 5.5l9 5"/>',
  agents:'<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  fleets:'<path d="M4 5h16v14H4zM8 2v6m8-6v6M8 12h8m-8 4h4"/>',
  messages:'<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 6 9 7 9-7"/>',
  workspaces:'<path d="M3 7h7l2 2h9v11H3V7ZM3 7V4h7l2 3"/><path d="M7 13h10M7 16h6"/>',
  presets:'<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/>',
  runs:'<path d="m9 5 11 7-11 7Z"/><path d="M4 5v14"/>',
  plugins:'<path d="M9 3v5m6-5v5M6 8h12v3a6 6 0 0 1-12 0ZM12 17v4"/>',
  capabilities:'<path d="M9 6h12M9 12h12M9 18h12M3 6l1 1 2-2M3 12l1 1 2-2M3 18h3"/>',
  events:'<path d="M3 12h4l3-8 4 16 3-8h4"/>',
  search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  arrow:'<path d="M5 12h14m-5-5 5 5-5 5"/>',
  storage:'<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 4 16 4 16 0V5M4 12c0 4 16 4 16 0"/>',
  plus:'<path d="M12 5v14M5 12h14"/>',
  colony:'<circle cx="12" cy="5" r="2.5"/><circle cx="5" cy="12" r="2.5"/><circle cx="19" cy="12" r="2.5"/><circle cx="12" cy="19" r="2.5"/><path d="M12 7.5v9M7.5 12h9"/>',
  inbox:'<path d="M3 13h5l2 3h4l2-3h5"/><path d="M5 5h14l2 8v6H3v-6Z"/>',
  deliveries:'<path d="M4 8h16v12H4Z"/><path d="M4 8l2-4h12l2 4M12 8v12M9 12h6"/>',
  researcher:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5M8 10.5h5M10.5 8v5"/>'
};
function icon(name, cls='') { return `<svg class="icon ${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${iconPaths[name]||iconPaths.overview}</svg>`; }
const filters = {agents:{query:'',state:'all'},runs:{query:'',state:'all'}};
let renderedPage = '', renderedMarkup = '', composing = false;
function filterBar(kind) {
  const f=filters[kind], states=kind==='agents'?['ready','busy','draining','suspended','terminated']:['queued','running','completed','failed','cancelled','interrupted'];
  return `<div class="toolbar"><label class="search-field">${icon('search')}<input id="directory-search" data-filter="query" type="search" value="${esc(f.query)}" placeholder="${kind==='agents'?'搜索实例 ID、Preset 或模型协议':'搜索运行 ID、实例或输入'}" aria-label="${kind==='agents'?'搜索实例':'搜索运行'}" autocomplete="off"></label><label class="state-field"><span>状态</span><select id="directory-state" data-filter="state" aria-label="筛选状态"><option value="all">全部状态</option>${states.map(state=>`<option value="${state}" ${f.state===state?'selected':''}>${labels[state]}</option>`).join('')}</select></label><span class="filter-note" role="status">${resultCount(kind)}</span>${f.query||f.state!=='all'?button('清除筛选','clear-filters'):''}</div>`;
}
function resultCount(kind) { const list=kind==='agents'?data.instances:data.runs;return `${filtered(kind,list).length} / ${list.length} 条结果`; }
function filtered(kind,list) { const f=filters[kind]||{query:'',state:'all'},q=f.query.trim().toLocaleLowerCase();return list.filter(item=>(f.state==='all'||item.state===f.state)&&(!q||[item.id,item.preset_name,item.snapshot?.protocol,item.instance_id,item.input].filter(Boolean).join(' ').toLocaleLowerCase().includes(q))); }
function directory(kind) {
  const source=kind==='agents'?data.instances:data.runs,list=filtered(kind,source);
  return list.length?(kind==='agents'?agentsTable(list):runsTable(list)):(source.length?empty('没有符合条件的结果','试试其他关键词，或使用上方的“清除筛选”。'):kind==='agents'?agentsTable([]):runsTable([]));
}
function recentRuns(list) {
  return list.length?`<div class="recent-runs">${list.map(r=>`<button class="recent-run" data-action="run-detail" data-id="${esc(r.id)}"><span class="run-symbol ${esc(r.state)}">${icon('runs')}</span><span class="recent-run-copy"><span class="recent-run-title">${esc(r.input)}</span><span class="recent-run-meta"><span class="mono">${esc(short(r.id))}</span><span>${time(r.started_at||r.created_at)}</span></span></span>${badge(r.state)}<span class="row-arrow">↗</span></button>`).join('')}</div>`:empty('等待第一次执行','从就绪实例提交任务，在这里检查执行证据。',button('查看实例','goto-agents'));
}

function toast(message) { $('#toast').textContent = message; $('#toast').classList.add('visible'); clearTimeout(toastTimer); toastTimer = setTimeout(() => $('#toast').classList.remove('visible'), 4000); }
async function api(path, body, method = 'POST', key = null) {
  const options = {credentials:'same-origin', headers:{}};
  if (body !== undefined) { options.method = method; options.headers['Content-Type'] = 'application/json'; options.body = JSON.stringify(body); }
  if (key) options.headers['Idempotency-Key'] = key;
  let response;
  try { response = await fetch(path, options); }
  catch { throw new Error('无法连接本地平台。请检查服务是否运行，再重试。'); }
  let value;
  try { value = await response.json(); } catch { throw new Error(`HTTP ${response.status}`); }
  if (response.status === 401) { authenticated = false; if (!$('#login-dialog').open) $('#login-dialog').showModal(); }
  if (!response.ok) {
    const code=value.error?.code || value.rejection || `HTTP ${response.status}`;
    const messages={process_plugins_disabled:'请使用 --allow-process-plugins 启动 daemon 后重试。',plugin_disabled:'插件版本已禁用，请选择其他版本或重新检查并启用。',plugin_digest_mismatch:'文件内容与 manifest 摘要不一致，修正包内容后重试。',plugin_cache_tampered:'缓存文件已变化，平台已拒绝执行。',invalid_plugin_manifest:'请检查 manifest 协议、工具 schema、相对路径与文件摘要。',plugin_source_unreadable:'无法读取服务器上的包目录，请填写绝对路径。',plugin_kind_conflict:'此 ID 已被另一种插件占用，请更换命名空间。',invalid_fleet_plan:'请检查Agent 池数量、策略与 TTL。',fleet_revision_conflict:'Agent 池已被其他操作修改，请关闭表单并刷新后重试。',fleet_operation_in_progress:'已有操作正在协调，完成后可再次扩缩容。',fleet_terminal:'Agent 池已进入终止流程，无法恢复或扩缩容。',invalid_fleet_action:'请检查目标数量与操作参数。',invalid_ttl:'TTL 须为 1 秒至 30 天之间的整数。',unauthorized:'访问令牌无效或会话已过期，请重新登录。',preset_version_immutable:'这个 Preset 版本已发布，请更换版本号。',plugin_version_immutable:'这个插件版本已安装，请更换版本号。',instance_not_ready:'实例当前无法接收任务，请刷新并选择就绪实例。',instance_quota_exceeded:'容量已满，请减少数量，或缩容/终止Agent 池以释放预留。',invalid_transition_or_instance_busy:'实例状态已变化或仍在执行，请刷新后重试。',idempotency_conflict:'上次请求的内容已更改，请关闭后重新打开表单提交。',invalid_preset:'名称和版本号须为 1–80 个英文字母、数字、点、下划线或连字符。',invalid_prompt_bundle:'请检查插件 ID、版本号和必填内容。'};
    Object.assign(messages,{invalid_preset_definition:'Preset 定义包含未知字段或字段类型不正确。',parent_preset_not_found:'父级 Preset 版本不存在，请刷新目录后重新选择。',invalid_preset_parent:'父级引用无效；Preset 不能继承自身。',preset_inheritance_depth_or_cycle:'继承链超过 16 层或包含循环，请选择更上层的 Preset。',duplicate_plugin_binding:'同一插件 ID 只能绑定一个版本，请取消重复选择。',ambiguous_plugin_bindings:'不能同时提交 plugin 和 plugins。',too_many_plugin_bindings:'每个 Preset 最多绑定 16 个插件。',preset_capability_limit:'能力锁超过 64 个工具或插件指令超过 64,000 UTF-8 字节，请减少绑定。',invalid_parameters_schema:'参数 schema 不受支持或超出结构上限。',parameters_schema_must_be_closed_object:'参数 schema 根须为 object，且 additionalProperties=false。',unknown_default_parameter:'默认值中存在未声明的参数。',invalid_default_parameter:'默认值的类型或范围不符合参数 schema。',invalid_parameter_names:'参数最多 32 个；名称限英文字母、数字、点、下划线和连字符。',unknown_template_parameter:'模板引用了未声明的参数，请添加同名参数或修改模板。',template_parameter_must_be_required_or_defaulted:'模板引用的参数必须设为必填或提供默认值。',invalid_parameter_template:'模板须使用配对的 {{参数名}}，不支持表达式。',parameters_schema_required:'填写默认值前须声明参数 schema。',parameters_not_declared:'该 Preset 未声明参数，不能传入参数值。',invalid_instance_parameters:'参数缺失、类型不正确或超出 schema 范围，请检查必填、类型和枚举值。',parameters_too_large:'合并默认值后的参数不能超过 16 KiB 序列化内容。',preset_snapshot_too_large:'完整 Preset 与能力快照超过 1 MiB，请减少绑定或 schema 内容。',preset_field_required:'独立定义必须填写用途、目标和系统指令。',preset_protocol_required:'请选择模型协议，或选择可继承的父级 Preset。',preset_field_cannot_be_null:'除旧版 plugin 外，Preset 字段不能为 null；省略字段表示继承。'});
    Object.assign(messages,{invalid_message:'消息不能为空，最多 8,000 个 UTF-8 字节；请检查关联 ID。',message_recipient_terminal:'接收实例已停止接收消息，请选择其他身份。',message_backlog_full:'消息积压已达上限；请等待消费或丢弃待处理消息。',message_action_state_conflict:'状态已变化，无法执行此处置；请刷新消息详情。',invalid_message_action:'请填写处置说明，最多 2,000 个 UTF-8 字节。',message_recipient_out_of_scope:'接收实例不在同一板根任务范围内，无法接收该消息。',message_causal_context_conflict:'关联 ID 与前置消息不一致。',message_hop_limit:'消息因果链已达 8 层，请检查是否存在回复循环。',message_correlation_quota_exceeded:'该关联链已达 128 条消息上限。',causation_message_not_found:'前置消息不存在，请核对完整 ID。',message_not_found:'消息不存在，请刷新列表。'});
    Object.assign(messages,{invalid_board_review:'退回修改必须填写评审意见，最多 8,000 个 UTF-8 字节。',board_review_result_changed:'成果已变化，请刷新任务后评审最新结果。',board_result_already_reviewed:'本轮结果已经评审，请刷新任务。',board_review_root_required:'请在根任务汇总结果后进行评审。',invalid_board_task:'请填写任务名称、内容和验收标准；名称最多 200、内容和验收标准各 8,000 个 UTF-8 字节。',board_root_limit:'活动任务已达上限，请等待已有任务完成或取消后再发布。',board_task_not_found:'任务不存在，请返回任务板刷新。',board_root_not_active:'这项任务已结束，无法继续操作。'});
    throw new Error(messages[code] || `操作未完成（${code}），请检查配置后重试。`);
  }
  return value;
}
// 旧 hash 全部重定向，保持历史链接可用；默认首页是人类观察者的工作现场。
const pageRedirects={overview:'studio',agents:'researcher',fleets:'researcher',messages:'researcher',runs:'researcher',events:'researcher',capabilities:'researcher',workspaces:'deliveries'};
function page() {
  if(boardId())return 'board';
  if(fleetId())return 'researcher';
  const hash=location.hash.slice(1);
  if(pageRedirects[hash])return pageRedirects[hash];
  return ['studio','colony','deliveries','board','inbox','presets','plugins','researcher'].includes(hash)?hash:'studio';
}
function applyPageRedirect(){
  const hash=location.hash.slice(1);
  if(pageRedirects[hash])location.replace(`#${pageRedirects[hash]}`);
  else if(hash.startsWith('fleet/'))location.replace('#researcher');
}
function head(title, buttons = '') { return `<div class="page-head"><div><h1>${title}</h1></div>${buttons?`<div class="head-actions">${buttons}</div>`:''}</div>`; }
function button(label, action, primary = false, extra = '') { return `<button type="button" class="button ${primary ? 'primary' : ''}" data-action="${action}" ${extra}>${label}</button>`; }
function empty(title, text, action = '') { return `<div class="empty"><b>${title}</b><p>${text}</p>${action}</div>`; }
function panel(title, body, link = '', suffix = '') { return `<section class="panel"><div class="panel-head"><h2 class="panel-title">${title}${suffix}</h2>${link}</div>${body}</section>`; }

function table(headers, rows) { return `<div class="table-wrap" tabindex="0" role="region" aria-label="${esc(headers.join('、'))}，可横向滚动"><table class="${['操作','检查','载荷'].includes(headers.at(-1))?'table-actions':''}"><thead><tr>${headers.map(h=>`<th scope="col">${h}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`; }
function agentRows(list) {
  return list.map((a,i) => `<tr><td><div class="table-name"><span class="agent-glyph">${icon('agents')}</span><div><div class="primary-text">${esc(a.preset_name)} <span class="mono">#${esc(a.id.slice(-5))}</span></div><div class="secondary-text mono">${esc(short(a.id))}</div></div></div></td><td>${badge(a.state)}</td><td><span class="mono">${esc(a.preset_version)}</span></td><td><span class="protocol">${esc(a.snapshot.protocol)}</span></td><td>${time(a.created_at)}</td><td><button class="text-button" data-action="agent-detail" data-id="${esc(a.id)}">检查 ↗</button>${a.state === 'ready' ? `<button class="text-button" data-action="run-form" data-id="${esc(a.id)}">运行</button>` : ''}</td></tr>`);
}
function agentsTable(list,compact=false) {
  if(compact && list.length) return '<div class="compact-agents">'+table(['实例 / Preset','状态','协议','操作'],list.map(a=>`<tr><td><div class="primary-text">${esc(a.preset_name)}</div><div class="secondary-text mono">${esc(short(a.id))}</div></td><td>${badge(a.state)}</td><td class="protocol">${esc(a.snapshot.protocol)}</td><td><button class="text-button" data-action="agent-detail" data-id="${esc(a.id)}">查看实例</button></td></tr>`))+'</div>';
  return list.length ? table(['AGENT / 实例','状态','Preset 版本','模型协议','创建时间','操作'],agentRows(list)) : empty('还没有 Agent 实例','选择一个 Preset，批量创建你的第一组实例。', button('＋ 创建实例','spawn',true)); }
function runsTable(list) { return list.length ? table(['RUN / 运行','Agent','输入','状态','开始时间','操作'],list.map(r=>`<tr><td class="mono">${esc(short(r.id))}</td><td class="mono">${esc(short(r.instance_id))}</td><td class="run-input" title="${esc(r.input)}">${esc(r.input.slice(0,55))}</td><td>${badge(r.state)}</td><td>${time(r.started_at || r.created_at)}</td><td><button class="text-button" data-action="run-detail" data-id="${esc(r.id)}">检查 ↗</button></td></tr>`)) : empty('尚无执行记录','创建实例并提交一条输入，即可查看工具循环与持久回执。'); }
function activity(list) { return list.length ? `<div class="activity">${list.map(e=>`<div class="activity-item"><span class="event-dot"></span><div><div class="activity-kind mono">${esc(e.kind)}</div><div class="activity-meta mono">${esc(short(e.subject))} #${e.seq}</div></div><span class="activity-time">${time(e.at)}</span></div>`).join('')}</div>` : empty('暂无事件','操作控制台后，持久事件将出现在这里。'); }
function overview() {
  const active = data.instances.filter(a=>a.state !== 'terminated'), busy = data.instances.filter(a=>a.state === 'busy'), complete = data.runs.filter(r=>r.state === 'completed');
  const review=(data.board?.tasks||[]).filter(t=>!t.parent_id&&t.review_state==='pending'),failed=data.runs.filter(r=>['failed','interrupted'].includes(r.state));
  const stats = [
    ['执行中实例',busy.length,`/ ${active.length} 个未终止`,'agents','检查实例与运行状态'],
    ['待评审任务',review.length,'项','board','核对结果与验收标准'],
    ['失败 / 结果未知',failed.length,'次','runs','最近 100 条运行'],
    ['已完成运行',complete.length,'次','runs','最近 100 条运行']
  ];
  return head('平台总览', button('发布任务','board-publish',true))+
    `<div class="stats">${stats.map(([label,count,unit,target,detail])=>`<a class="stat" href="#${target}"><span class="stat-label">${label}</span><div class="stat-number">${count}<small>${unit}</small></div><div class="stat-detail">${detail}</div></a>`).join('')}</div>`+
    `<div class="split"><div>${panel('未终止的 Agent',agentsTable([...busy,...active.filter(a=>a.state!=='busy')].slice(0,5),true),'<a href="#agents" class="panel-link">全部实例</a>',`<small>${active.length} 个</small>`)}${panel('最近执行',recentRuns(data.runs.slice(0,4)),'<a href="#runs" class="panel-link">全部执行记录</a>')}</div><div>${panel('模型配置',`<div class="panel-body">${data.models.map(m=>`<div class="model-row"><div><p>${esc(m.protocol)}</p><small>${esc(m.model || '未配置模型')}</small></div>${badge(m.configured?'good':'neutral',m.configured?'已配置':'未配置')}</div>`).join('')}<p class="hint">配置表示凭证已加载；实际连通情况见执行回执。</p></div>`)}${panel('最近活动',`<div class="panel-body">${activity(events.slice(-5).reverse())}</div>`,'<a href="#events" class="panel-link">全部事件</a>')}</div></div>`;

}
function progressPage() {
  if(!data.capabilities.length)return head('实现进度')+empty('尚未读取到能力清单','请刷新页面；持续为空时检查平台 API 的能力清单。');
  const counts = status => data.capabilities.filter(c=>c.status===status).length;
  const pathMeta=[['emergence','涌现机制','北极星目标路径：stigmergy 痕迹、分工、自组织认领与整合'],['foundation','运行底座','支撑个体执行与持久状态'],['verification','证据与验证','可信交付与返工'],['support','支撑能力','入口、可视化与生态'],['governance','治理与规模化','预算、授权、隔离与远程，按需推进']];
  const item=c=>`<article class="progress-item"><div class="progress-title"><h3>${esc(c.name)}</h3>${badge(c.status)}</div><p>${esc(c.detail)}</p><details><summary>验证与代码位置</summary><dl class="detail-kv">${c.scope?`<dt>范围</dt><dd>${esc(({current:'本轮重点',extension:'后续扩展',deferred:'暂缓'})[c.scope]||c.scope)}</dd>`:''}<dt>验证</dt><dd>${esc(({pending:'本轮待验证','native-tests-and-browser':'原生测试与 Web 联调'})[c.verification]||c.verification||'未记录')}</dd><dt>当前代码</dt><dd>${c.source.length?c.source.map(path=>`<code class="wrap">${esc(path)}</code>`).join('<br>'):'尚无实现模块'}</dd><dt>规划目录</dt><dd>${c.target.map(path=>`<code class="wrap">${esc(path)}</code>`).join('<br>')}</dd></dl></details></article>`;
  return head('实现进度')+
    `<div class="progress-summary">${['implemented','partial','planned'].map(status=>`<span>${labels[status]} <strong>${counts(status)}</strong> 项</span>`).join('')}</div>`+
    pathMeta.map(([key,title,desc])=>{const group=data.capabilities.filter(c=>c.path===key);if(!group.length)return '';return panel(`${title} <small class="hint">${esc(desc)}</small>`,`<div class="progress-list">${group.map(item).join('')}</div>`,`<span class="board-count">${group.filter(c=>c.status==='implemented').length}/${group.length} 已实现</span>`);}).join('');
}
function render() {
  if (!data || composing) return;
  const p=page(), names={studio:'工作现场',colony:'蚁群',deliveries:'成果库',board:'任务板',inbox:'需要你处理',presets:'Presets',plugins:'插件与能力',researcher:'研究者视图'};
  const inboxCount=typeof inboxItems==='function'&&data?inboxItems().length:0;
  const inboxBadge=$('#inbox-count');
  if(inboxBadge){inboxBadge.textContent=inboxCount;inboxBadge.hidden=!inboxCount;}
  document.querySelectorAll('nav a').forEach(a=>{a.classList.toggle('active',a.dataset.page===p);if(a.dataset.page===p)a.setAttribute('aria-current','page');else a.removeAttribute('aria-current');});
  document.title=`${names[p]} Ant³`;
  let html='';
  if(p==='board') html=`<div class="board-page">${boardPage()}</div>`;
  if(p==='studio') html=studioPage();
  if(p==='colony') html=colonyPage();
  if(p==='deliveries') html=deliveriesPage();
  if(p==='inbox') html=inboxPage();
  if(p==='researcher') html=researcherPage();
  if(p==='presets') html=head('Presets', button('发布 Preset','preset-form',true))+panel('Preset 版本',data.presets.length?table(['Preset / 版本','用途与目标','模型协议','操作'],data.presets.map(p=>`<tr><td><div class="primary-text">${esc(p.name)}</div><div class="secondary-text mono">${esc(p.version)}</div></td><td><div class="primary-text">${esc(p.definition.purpose)}</div><details class="table-detail"><summary>查看目标</summary><p class="record-text">${esc(p.definition.objective)}</p></details></td><td><span class="protocol">${esc(p.definition.protocol)}</span></td><td><button class="text-button" data-action="preset-detail" data-id="${esc(p.name)}" data-version="${esc(p.version)}">查看定义</button><button class="text-button" data-action="spawn" data-id="${esc(p.name)}" data-version="${esc(p.version)}">创建实例</button></td></tr>`)):empty('尚无 Preset 版本','点击“发布 Preset”，填写用途、目标和模型协议。'),`<span class="board-count">${data.presets.length} 个版本</span>`);
  if(p==='plugins') html=pluginsPage();
  // Do not replace an unchanged view during polling: preserve focus and scroll.
  if(renderedMarkup!==html) {
    const focused=document.activeElement, focusId=focused?.id, action=focused?.dataset.action, itemId=focused?.dataset.id;
    const range=focused instanceof HTMLInputElement && ['search','text'].includes(focused.type)?[focused.selectionStart,focused.selectionEnd]:null;
    const scrollSelector='.table-wrap, .studio-map-scroll, .studio-task-scroll, .studio-inspect, .studio-full-text';
    const scrolls=[...document.querySelectorAll(scrollSelector)].map(el=>[el.scrollLeft,el.scrollTop]);
    const openDetails=new Set([...$('#content').querySelectorAll('details[open][id]')].map(el=>el.id));
    $('#content').innerHTML=html;
    if(renderedPage===p)$('#content').querySelectorAll('details[id]').forEach(el=>{el.open=openDetails.has(el.id);});
    document.querySelectorAll(scrollSelector).forEach((el,i)=>{el.scrollLeft=scrolls[i]?.[0]||0;el.scrollTop=scrolls[i]?.[1]||0;});
    if(renderedPage===p && !$('#modal').open) {
      const restore=focusId?document.getElementById(focusId):action?[...$('#content').querySelectorAll('[data-action]')].find(el=>el.dataset.action===action&&el.dataset.id===itemId):null;
      if(restore){restore.focus({preventScroll:true});if(range)restore.setSelectionRange(...range);}
    }
  }
  renderedPage=p; renderedMarkup=html;
  studioSyncLabel();
}
async function refresh(force=false) {
  if(refreshTask){await refreshTask;if(!force)return;}
  refreshTask=readState();
  try {await refreshTask;} finally {refreshTask=null;}
}
async function readState() {
  try {
    data=await api('/api/v1/snapshot'); authenticated=true;
    if(boardId())await boardReadDetail();
    if(fleetId()) {
      try {selectedFleet=await api(`/api/v1/agent-fleets/${encodeURIComponent(fleetId())}`);}
      catch(error) {selectedFleet={id:fleetId(),loadError:error.message};}
    }
    // Drain a bounded page per tick; the cursor always advances only after a successful read.
    const chunk=await api(`/api/v1/events?after=${cursor}`);
    if(chunk.events.length){events=events.concat(chunk.events).slice(-500);cursor=chunk.events.at(-1).seq;}
    studio.syncedAt=Date.now();studio.disconnected=false;
    $('#connection').className='connection-indicator connected'; $('#connection').setAttribute('aria-label','内核已连接'); $('#connection').title='内核已连接'; $('#notice').textContent='';
    render();
  } catch(error) {
    studio.disconnected=true;studioSyncLabel();
    $('#connection').className='connection-indicator disconnected'; $('#connection').setAttribute('aria-label','连接未就绪'); $('#connection').title='连接未就绪，将自动重连';
    if(authenticated) $('#notice').textContent=`刷新失败：${error.message}。页面保留上次成功读取的状态。`;
  }
}
let modalTrigger=null, inspectionRequest=0;
function openModal(title, html) {
  inspectionRequest++;
  if(!$('#modal').open&&document.activeElement!==document.body)modalTrigger=document.activeElement;
  $('#modal-error').hidden=true;$('#modal-title').textContent=title;$('#modal-body').innerHTML=html;
  $('#modal').classList.toggle('inspector',Boolean($('#modal-body .inspector-summary')));
  const formKind=$('#modal-body form')?.dataset.form;
  $('#modal').classList.toggle('wide-form',['preset','spawn'].includes(formKind));
  $('#modal').classList.toggle('confirmation',['terminate','drain','run-cancel','board-cancel','process-state','message-action'].includes(formKind));
  $('#modal').setAttribute('role',$('#modal [data-form="terminate"], #modal [data-form="drain"]')?'alertdialog':'dialog');
  $('#modal-body').querySelectorAll('input[pattern]').forEach(input=>{input.maxLength=80;input.title='最多 80 个英文字母、数字、点、下划线或连字符';});
  if(!$('#modal').open)$('#modal').showModal();
  const focus=$('#modal form input:not(:disabled), #modal form select:not(:disabled), #modal form textarea')||$('#modal [data-action="close"]');
  focus.focus({preventScroll:true});$('#modal').scrollTop=0;
}
const fieldLabels={id:'标识',name:'名称',version:'版本',text:'文本',input:'输入',output:'输出',content:'内容',body:'内容',kind:'类型',state:'状态',status:'状态',error:'错误',code:'错误码',message:'说明',seq:'序号',sequence:'序号',at:'时间',created_at:'创建时间',started_at:'开始时间',finished_at:'结束时间',subject:'关联对象',data:'记录内容',arguments:'调用参数',call:'工具调用',calls:'工具调用',observations:'工具回执',changed:'有变更',run_id:'运行标识',instance_id:'实例标识',recipient_id:'接收实例',sender_instance_id:'发送实例',source_run_id:'来源运行',correlation_id:'关联标识',causation_id:'前置消息',purpose:'用途',objective:'目标',instructions:'指令',protocol:'协议',model:'模型',tools:'工具',parameters:'参数',properties:'字段',required:'必填字段',type:'类型',description:'说明',additionalProperties:'允许额外字段',enum:'可选值',minimum:'最小值',maximum:'最大值',minLength:'最短长度',maxLength:'最长长度',minItems:'最少项目',maxItems:'最多项目',items:'项目规则',schema:'参数规则',defaults:'默认值',actual:'实际参数',revision:'修订',turns:'会话记录',events:'事件',requests:'模型请求',report:'运行回执',snapshot:'固定配置',definition:'定义',publication:'发布定义',extends:'继承来源',lineage:'继承链',plugin_snapshot:'插件快照',plugin_snapshots:'插件快照',plugins:'插件',digest:'内容摘要',manifest:'插件契约',runtime:'运行方式',files:'文件',mutates:'声明有副作用',parallel_safe:'允许并行',input_schema:'输入规则',output_schema:'输出规则',acceptance:'验收标准',review:'验收记录',tasks:'任务',parent_id:'拆分来源',root_id:'根任务',key:'键',value:'值',usage:'模型用量',input_tokens:'输入 Token',output_tokens:'输出 Token',generation_ms:'模型生成耗时（ms）',execution_ms:'执行耗时（ms）',model_metadata:'模型信息',model_request:'模型请求',system:'系统指令',sender_kind:'发送方类型',delivery_state:'投递状态',inbox_state:'消费状态',resolution:'处置说明',resolved_at:'处置时间',received_at:'入 Inbox 时间',ack_at:'派发确认时间',depth:'因果深度',continuation:'供应商续接记录'};
function valueText(value) {
  if(value===null||value===undefined)return '未设置';
  if(typeof value==='boolean')return value?'是':'否';
  if(Array.isArray(value))return value.map(valueText).join('、')||'空列表';
  if(typeof value==='object')return Object.entries(value).map(([k,v])=>`${fieldLabels[k]||k}：${valueText(v)}`).join('；')||'无字段';
  return String(value);
}
function schemaSummary(schema){
  if(!schema||typeof schema!=='object')return '<span class="secondary-text">未声明</span>';
  if(schema.type==='object'&&schema.properties){
    const required=new Set(schema.required||[]);
    return `<dl class="record-fields">${Object.entries(schema.properties).map(([key,value])=>`<dt>${esc(fieldLabels[key]||key)}${required.has(key)?' <span class="hint">必填</span>':''}</dt><dd>${esc(value.type||'any')}${value.description?`<span class="hint"> · ${esc(value.description)}</span>`:''}</dd>`).join('')}</dl>`;
  }
  return `<span class="mono">${esc(schema.type||'any')}</span>`;
}
function dataView(value,depth=0) {
  if(depth>10)return '<span class="hint">嵌套记录较深，请通过 API 查询完整内容。</span>';
  if(typeof value==='string') {
    const text=value.trim();
    if(text.length<128000&&((text.startsWith('{')&&text.endsWith('}'))||(text.startsWith('[')&&text.endsWith(']')))) {
      try{return dataView(JSON.parse(text),depth+1);}catch{}
    }
    return `<span class="record-text">${esc(value)}</span>`;
  }
  if(value===null||typeof value!=='object')return `<span class="record-value">${esc(valueText(value))}</span>`;
  if(Array.isArray(value)) {
    if(!value.length)return '<span class="secondary-text">暂无记录</span>';
    if(value.every(v=>v===null||typeof v!=='object'))return `<ul class="record-values">${value.slice(0,100).map(v=>`<li>${dataView(v,depth+1)}</li>`).join('')}</ul>${value.length>100?'<p class="hint">显示前 100 项，其余请通过 API 查询。</p>':''}`;
    return `<div class="record-list">${value.slice(0,100).map((v,i)=>`<details class="record-item" ${value.length===1?'open':''}><summary>${esc(v?.name||v?.kind||v?.call?.name||v?.role||`记录 ${i+1}`)}</summary>${dataView(v,depth+1)}</details>`).join('')}${value.length>100?'<p class="hint">显示前 100 项，其余请通过 API 查询。</p>':''}</div>`;
  }
  const entries=Object.entries(value);if(!entries.length)return '<span class="secondary-text">无字段</span>';
  return `<dl class="record-fields">${entries.map(([k,v])=>`<dt>${esc(fieldLabels[k]||k)}</dt><dd>${dataView(v,depth+1)}</dd>`).join('')}</dl>`;
}
function identity(label,value) { return `<div class="identity"><span>${esc(label)}</span><code>${esc(value)}</code><button type="button" class="text-button" data-action="copy" data-value="${esc(value)}" aria-label="复制${esc(label)}">复制</button></div>`; }
function formEnd(label) { return `<div class="error" id="form-error" role="alert"></div><div class="form-actions">${button('取消','close')}<button class="button primary" type="submit">${label}</button></div>`; }
function spawnForm(name,version) {
  const selected=data.presets.find(p=>p.name===name&&p.version===version)||data.presets.find(p=>p.name==='researcher')||data.presets[0];
  if(!selected) return toast('请先发布一个 Preset');
  const limit=data.capacity.limit,used=data.capacity.reserved,available=data.capacity.available;
  openModal('从 Preset 创建实例',`<form data-form="spawn"><label>Preset 版本<select name="preset">${data.presets.map(p=>`<option value="${esc(JSON.stringify({name:p.name,version:p.version}))}" ${p===selected?'selected':''}>${esc(p.name)} @ ${esc(p.version)} ${esc(p.definition.protocol)}</option>`).join('')}</select></label><div id="spawn-parameters">${spawnParameterFields(selected.definition)}</div><div class="quota-summary"><span>实例配额</span><strong>${used} <span>/ ${limit} 已占用或预留</span></strong><span>还可创建 ${available} 个</span></div><label>Agent 池策略<select name="mode"><option value="finite">有限批次 终止后不补建</option><option value="maintain">持续维持 自动补足目标数量</option></select></label><p class="hint">持续维持会自动补建成员并重新计时 TTL。停止补建请缩容或终止 Agent 池。</p><label>实例数量<input name="count" type="number" min="1" max="${Math.max(1,available)}" value="${Math.min(3,available)||1}" ${available?'':'disabled'} required aria-describedby="spawn-hint"></label><label>实例 TTL（分钟，可选）<input name="ttl_minutes" type="number" min="1" max="43200" step="1" placeholder="不填则持续保留" aria-describedby="ttl-hint"></label><p class="hint" id="ttl-hint">从创建时开始计时，最多 30 天。到期停止接收任务、取消排队并协作取消运行；历史记录保留。</p><p class="hint" id="spawn-hint">${available?'创建后即可向就绪实例提交任务。':'当前容量已满。缩容或终止Agent 池可释放预留；finite 的闲置成员也可单独终止。'} Worker 最多并发 ${data.limits.workers} 个运行。</p>${formEnd('创建实例')}</form>`,'批量实例化');
  if(!available)$('#modal [type="submit"]').disabled=true;
}

function runForm(id) {
  const a=data.instances.find(item=>item.id===id);if(!a)return;
  const model=data.models.find(m=>m.protocol===a.snapshot.protocol);
  openModal('提交任务',`<form data-form="run" data-id="${esc(id)}"><div class="task-context"><span class="agent-glyph">${icon('agents')}</span><div><strong>${esc(a.preset_name)} <span class="mono">@ ${esc(a.preset_version)}</span></strong><small>${esc(model?.model||a.snapshot.protocol)} ${esc(a.snapshot.protocol)}</small></div>${badge(a.state)}</div>${identity('实例 ID',id)}<label>任务内容<textarea name="text" required maxlength="64000" rows="6" placeholder="描述需要 Agent 完成的任务…" aria-describedby="run-hint"></textarea></label><p class="hint" id="run-hint">${model?.configured?'此任务将调用真实模型，按供应商规则计费。':'当前协议未配置模型，任务无法正常执行。请先配置模型并重启服务。'} 每次最多 12 轮、24 次工具调用、120 秒。</p>${formEnd('提交任务')}</form>`,'独立会话执行');
}
async function agentDetail(id) {
  const a=data.instances.find(a=>a.id===id); if(!a) return;
  const request=++inspectionRequest;
  const session=await api(`/api/v1/agent-instances/${encodeURIComponent(id)}/session`);
  if(request!==inspectionRequest)return;
  modalContext={type:'agent',id,session};
  const actions=a.state==='ready'?button('提交运行','run-form',true,`data-id="${esc(id)}"`)+button('暂停','suspend',false,`data-id="${esc(id)}"`):a.state==='suspended'?button('恢复','resume',true,`data-id="${esc(id)}"`):'';
  openModal(a.preset_name,`<div class="inspector-summary"><div><span>生命周期</span>${badge(a.state)}</div><div><span>Preset 版本</span><strong class="mono">${esc(a.preset_version)}</strong></div><div><span>会话记录</span><strong>${session.turns.length} <small>项</small></strong></div></div>${identity('实例 ID',id)}${a.expires_at?`<p class="hint">TTL 截止：${esc(new Date(a.expires_at).toLocaleString('zh-CN'))} 到期将停止接收任务并请求取消活动运行。</p>`:''}<div class="inspector-actions">${button('所属 Agent 池','fleet-nav',false,`data-id="${esc(a.fleet_id)}"`)}${actions}${['ready','busy','suspended'].includes(a.state)?`<button class="text-button" data-action="drain" data-id="${esc(id)}">排空并终止</button>`:''}${['ready','suspended'].includes(a.state)?`<button class="text-button danger-action" data-action="terminate" data-id="${esc(id)}">终止实例</button>`:''}</div><div id="detail-view">${definitionView(a.snapshot)}</div><details class="raw-data"><summary>Agent 池与存储信息</summary><dl class="detail-kv"><dt>Agent 池</dt><dd class="mono">${esc(a.fleet_id)}</dd><dt>槽位 / 代际</dt><dd>${a.slot??'—'} / ${a.generation??'—'}</dd><dt>截止时间</dt><dd>${a.expires_at?esc(new Date(a.expires_at).toLocaleString('zh-CN')):'未设置 TTL'}</dd><dt>检查点</dt><dd>${session.events.length}</dd><dt>模型协议</dt><dd>${esc(a.snapshot.protocol)}</dd></dl></details>`,'Agent 实例');
}
function definitionView(definition) {
  const bindings=presetBindings(definition);
  return `<dl class="definition"><dt>主要功能</dt><dd>${esc(definition.purpose)}</dd><dt>预期目标</dt><dd>${esc(definition.objective)}</dd><dt>模型协议</dt><dd class="mono">${esc(definition.protocol)}</dd>${definition.extends?`<dt>继承来源</dt><dd class="mono">${esc(definition.extends.name)} @ ${esc(definition.extends.version)} ${definition.lineage.length} 层</dd>`:''}<dt>系统指令</dt><dd class="instructions">${esc(definition.instructions)}</dd></dl>${definition.parameters?`<dl class="detail-kv"><dt>实际参数</dt><dd>${Object.entries(definition.parameters).map(([key,value])=>`<div><strong class="mono">${esc(key)}</strong> ${esc(valueText(value))}</div>`).join('')||'空参数对象'}</dd></dl>`:''}${bindings.length?`<dl class="definition"><dt>能力快照</dt><dd>${bindings.map(p=>`<div><strong class="mono">${esc(p.id)} @ ${esc(p.version)}</strong> ${p.kind==='organization'?'内置组织能力':p.kind==='process'?'外部进程工具':'提示词包'}${p.digest?`<p class="hint mono wrap">${esc(p.digest)}</p>`:''}${p.kind==='process'?`<p class="hint">${p.manifest.tools.map(t=>esc(`${p.id}.${t.name}`)).join('、')}</p>`:''}</div>`).join('')}</dd></dl>`:''}${definition.parameters_schema?`<dl class="definition"><dt>可配置参数</dt><dd>${schemaSummary(definition.parameters_schema)}${definition.parameter_defaults&&Object.keys(definition.parameter_defaults).length?`<p class="hint">默认值：${Object.entries(definition.parameter_defaults).map(([key,value])=>`${esc(key)} = ${esc(valueText(value))}`).join('，')}</p>`:''}</dd></dl>`:''}`;
}
async function runDetail(id, preserve=false) {
  const request=++inspectionRequest;
  const r=await api(`/api/v1/runs/${encodeURIComponent(id)}`);
  const selected=preserve&&modalContext?.id===id?modalContext.tab||'output':'output';
  const scroll=preserve?$('#modal').scrollTop:0;
  if(request!==inspectionRequest)return;
  modalContext={type:'run',id,run:r,tab:selected};
  const duration=r.report?.execution_ms ?? (r.finished_at&&r.started_at?r.finished_at-r.started_at:null);
  const usage=r.report?.usage?`${(r.report.usage.input+r.report.usage.output).toLocaleString()} tokens`:'未记录';
  openModal('执行详情',`<div class="inspector-summary"><div><span>运行结果</span>${badge(r.state)}</div><div><span>耗时</span><strong>${durationText(duration)}</strong></div><div><span>模型用量</span><strong>${usage}</strong></div></div><div class="inspector-actions">${['queued','running'].includes(r.state)?`<button class="button danger" data-action="cancel-run" data-id="${esc(id)}">取消运行</button>`:''}${['queued','running'].includes(r.state)?button('刷新详情','refresh-run',false,`data-id="${esc(id)}" title="列表每 1.5 秒自动同步"`):''}</div><div class="tabs">${[['output','结果'],['report','执行过程']].map(([key,label])=>`<button data-action="run-tab" data-tab="${key}" aria-pressed="${key===selected}" class="${key===selected?'active':''}">${label}</button>`).join('')}</div><div id="detail-view">${runTabView(r,selected)}</div><details class="raw-data"><summary>技术标识</summary><dl class="detail-kv"><dt>Run</dt><dd class="mono wrap">${esc(id)}</dd><dt>实例</dt><dd class="mono wrap">${esc(r.instance_id)}</dd></dl></details>`,'运行与证据');
  if(preserve){$('#modal [data-action="refresh-run"]').focus({preventScroll:true});$('#modal').scrollTop=scroll;}
}
function runTabView(r,tab) {
  if(tab==='report')return reportView(r.report);
  return runOutput(r);
}
function reportView(report) {
  if(!report)return empty('过程尚未生成','运行结算后可以查看执行过程。');
  const observations=report.observations||[];
  if(!observations.length)return empty('没有工具调用','这次运行直接给出了结果，没有使用工具。');
  return `<div class="receipt-summary"><span>${icon('capabilities')} ${observations.length} 个步骤</span><span>${durationText(report.execution_ms)}</span></div><ol class="run-steps">${observations.map((item,i)=>`<li><span class="run-step-index">${String(i+1).padStart(2,'0')}</span><div class="run-step-copy"><strong>${esc(toolLabel(item.call.name))}</strong><p>${esc(toolSummary(item))}</p></div></li>`).join('')}</ol>${report.error?`<div class="callout">${esc(report.error.message||runErrorText(report.error.code))}</div>`:''}<p class="hint">每一步的参数与回执原始记录在研究者视图的执行记录中查看。</p>`;
}
function runOutput(r) {
  const error=r.report?.error,pending=['queued','running'].includes(r.state);
  const result=error?`<div class="execution-error" role="status"><strong>本次执行未完成</strong><p>${esc(error.message||runErrorText(error.code))}</p></div>`:pending?`<div class="execution-pending">${icon('runs')}<div><strong>${r.state==='queued'?'任务已进入队列':'Agent 正在执行'}</strong><p>点击“刷新详情”检查最新结果；关闭面板不会中断任务。</p></div></div>`:`<div class="output">${esc(r.report?.output??(r.state==='cancelled'?'运行已取消，暂无输出。':'此运行未提供文本输出。'))}</div>`;
  return `<div class="output-section"><h3>执行结果</h3>${result}</div><details class="raw-data" ${pending?'open':''}><summary>用户输入</summary><div class="output">${esc(r.task_input??r.input)}</div></details>`;
}
async function handleAction(action, element) {
  if(action==='navigation-toggle') {
    const open=element.getAttribute('aria-expanded')!=='true';
    element.setAttribute('aria-expanded',String(open));$('#shell').classList.toggle('navigation-open',open);
    if(open)$('nav a[aria-current="page"]')?.focus();
    return;
  }
  if(await studioAction(action,element))return;
  if(await colonyAction(action,element))return;
  if(await processAction(action,element))return;
  const id=element.dataset.id;
  if(action==='fleet-nav') {$('#modal').close();location.hash=`fleet/${id}`;return;}
  if(action==='fleet-scale'||action==='fleet-stop')return fleetActionForm(id,action==='fleet-stop');
  if(action==='copy') {
    try {await navigator.clipboard.writeText(element.dataset.value);element.textContent='已复制';setTimeout(()=>{if(element.isConnected)element.textContent='复制';},1800);}
    catch {throw new Error('浏览器未允许复制，请选中 ID 后手动复制。');}return;
  }
  if(action==='goto-agents') {location.hash='researcher';return;}
  if(action==='clear-filters') {const kind=page()==='researcher'?(researcherSections.has('agents')?'agents':'runs'):page();if(!filters[kind])return;filters[kind]={query:'',state:'all'};render();$('#directory-search')?.focus();return;}
  if(await workspaceAction(action,element))return;
  if(await boardAction(action,element))return;
  if(await messageAction(action,element))return;
  if(action==='close') {if(!$('#modal form[aria-busy="true"]'))$('#modal').close();return;}
  if(action==='capabilities') {location.hash='researcher';return;}
  if(action==='spawn') return spawnForm(id,element.dataset.version);
  if(action==='preset-form') return presetForm();
  if(action==='run-form') return runForm(id);
  if(action==='agent-detail') return agentDetail(id);
  if(action==='run-detail') return runDetail(id);
  if(action==='refresh-run') {await refresh(true);return runDetail(id,true);}
  if(action==='preset-detail') {const p=data.presets.find(p=>p.name===id&&p.version===element.dataset.version);return openModal(p.name,definitionView(p.definition),`Preset 版本 ${p.version}`);}
  if(action==='plugin-detail') {
    const p=data.plugins.find(p=>p.id===id&&p.version===element.dataset.version);
    return openModal(p.id,`<div class="inspector-summary"><div><span>版本</span><strong>${esc(p.version)}</strong></div><div><span>类型</span><strong>提示词包</strong></div></div><dl class="definition"><dt>说明</dt><dd>${esc(p.definition.description)}</dd><dt>指令</dt><dd class="instructions">${esc(p.definition.instructions)}</dd></dl>${identity('内容摘要',p.digest)}`,'插件定义');
  }
  if(action==='event-detail') {
    const e=events.find(e=>String(e.seq)===id);if(!e)return;
    const narrative=typeof window.swarmNarrative==='function'?window.swarmNarrative(e,data):'';
    return openModal('事件详情',`<div class="inspector-summary"><div><span>时间</span><strong>${time(e.at)}</strong></div><div><span>类别</span><strong>${esc(eventLabel(e.kind))}</strong></div></div>${narrative?`<p class="form-intro">${esc(narrative)}</p>`:''}<dl class="detail-kv"><dt>对象</dt><dd class="mono wrap">${esc(e.subject)}</dd><dt>事件键</dt><dd class="mono wrap">${esc(e.kind)}</dd></dl><details class="raw-data"><summary>原始字段（技术）</summary>${dataView(e)}</details>`,'系统事件');
  }
  if(['suspend','resume','terminate','drain'].includes(action)) {
    if(action==='drain')return openModal('排空并终止实例？',`<form data-form="drain" data-id="${esc(id)}"><p class="form-intro">立即停止接收新任务，等待已接纳的排队或运行任务结算后终止。没有活动任务时立即终止。历史记录保留；此操作无法恢复。若属于持续维持组，退休后会创建新成员补足目标数量。</p>${identity('实例 ID',id)}${formEnd('确认排空')}</form>`,'生命周期操作');
    if(action==='terminate')return openModal('终止这个实例？',`<form data-form="terminate" data-id="${esc(id)}"><p class="form-intro">终止后，此实例无法恢复，历史记录保留。若属于持续维持组，平台会创建新实例补足目标数量；停止补建请终止整个Agent 池或缩容。</p>${identity('实例 ID',id)}${formEnd('确认终止实例')}</form>`,'生命周期操作');
    await api(`/api/v1/agent-instances/${encodeURIComponent(id)}/actions`,{action});$('#modal').close();await refresh();return toast('实例状态已更新');
  }
  if(action==='cancel-run') return openModal('取消这个运行？',`<form data-form="run-cancel" data-id="${esc(id)}"><p class="form-intro">排队任务将停止派发；已开始的运行会协作收尾。已经发生的工具调用和外部效果无法撤销，历史记录保留。</p>${identity('Run ID',id)}${formEnd('确认取消运行')}</form>`,'运行操作');
  if(action==='run-tab') {
    const r=modalContext.run, tab=element.dataset.tab;
    modalContext.tab=tab;
    $('#detail-view').innerHTML=runTabView(r,tab);
  }
  if(['agent-tab','run-tab'].includes(action)) element.parentElement.querySelectorAll('button').forEach(b=>{b.classList.toggle('active',b===element);b.setAttribute('aria-pressed',String(b===element));});
}
document.addEventListener('click',async event=>{
  if(event.target.closest('nav a,.brand'))closeNavigation();
  const anchor=event.target.closest('[data-task-anchor]');
  if(anchor){event.preventDefault();const heading=document.getElementById(`task-heading-${anchor.dataset.taskAnchor}`);if(heading){heading.tabIndex=-1;heading.focus({preventScroll:true});heading.scrollIntoView({block:'center'});}return;}
  const element=event.target.closest('[data-action]');if(!element)return;
  if(element.closest('form')?.getAttribute('aria-busy')==='true')return;
  if(element.disabled)return;
  if(!$('#modal').open)modalTrigger=element;
  element.disabled=true;element.setAttribute('aria-busy','true');
  try {await handleAction(element.dataset.action,element);} catch(error){
    if($('#modal').open){$('#modal-error').textContent=error.message;$('#modal-error').hidden=false;}
    else toast(error.message);
  } finally {element.disabled=false;element.removeAttribute('aria-busy');}
});
document.addEventListener('invalid',event=>{
  const field=event.target,form=field.form;
  if(!form?.dataset.form)return;
  const label=field.closest('label')?.firstChild.textContent.trim()||fieldLabels[field.name]||'此项';
  if(field.validity.valueMissing)field.setCustomValidity(`${field.tagName==='SELECT'?'请选择':'请填写'}${label}。`);
  else if(field.validity.patternMismatch)field.setCustomValidity(`${label}只能包含英文字母、数字、点、下划线或连字符，最多 80 个字符。`);
},true);
for(const type of ['input','change'])document.addEventListener(type,event=>{
  if(event.target.form?.dataset.form&&typeof event.target.setCustomValidity==='function')event.target.setCustomValidity('');
});
document.addEventListener('submit',async event=>{
  const form=event.target;
  if(form.id==='login-form') {
    event.preventDefault();
    if(form.getAttribute('aria-busy')==='true')return;
    form.setAttribute('aria-busy','true');
    const submit=form.querySelector('[type="submit"]');submit.disabled=true;submit.textContent='正在连接…';
    try {await api('/api/login',{token:$('#token').value.trim()});$('#token').value='';$('#login-error').textContent='';$('#login-dialog').close();await refresh();}
    catch(error){$('#login-error').textContent=error.message;}
    finally {submit.disabled=false;submit.textContent='进入控制台';form.removeAttribute('aria-busy');}
    return;
  }
  if(!form.dataset.form)return;
  event.preventDefault();if(form.getAttribute('aria-busy')==='true')return;
  const byteLimits={purpose:2000,objective:8000,instructions:32000,description:2000,text:64000,review:8000};
  for(const [name,limit] of Object.entries(byteLimits)) {
    const field=form.elements.namedItem(name);
    if(field&&new TextEncoder().encode(field.value).length>limit){$('#form-error').textContent=`内容过长：${field.closest('label').firstChild.textContent.trim()}最多 ${limit.toLocaleString()} 个 UTF-8 字节。`;field.focus();return;}
  }
  const values=Object.fromEntries(new FormData(form)),submit=form.querySelector('[type="submit"]'),label=submit.textContent;
  submit.disabled=true;submit.textContent='正在提交…';form.setAttribute('aria-busy','true');$('#form-error').textContent='';
  // Keep the same key when a network failure leaves acceptance uncertain.
  form.dataset.commandKey ||= crypto.randomUUID();
  try {
    let result;
    if(form.dataset.form==='run-cancel')result=await api(`/api/v1/runs/${encodeURIComponent(form.dataset.id)}/cancel`,{});
    if(form.dataset.form==='board-publish') {
      for(const name of ['title','instructions','acceptance'])if(new TextEncoder().encode(values[name]).length>(name==='title'?200:8000))throw new Error(`${name==='title'?'任务名称':'任务内容或验收标准'}超过 UTF-8 字节上限。`);
      const payload={title:values.title,instructions:values.instructions,acceptance:values.acceptance};
      if(values.verify_preset){const [name,version]=values.verify_preset.split('@');payload.verification={preset_name:name,preset_version:version,max_auto_reworks:Number(values.verify_reworks||'1')};}
      if(values.energy_budget)payload.energy_budget=Number(values.energy_budget);
      result=await api('/api/board/tasks',payload,'POST',form.dataset.commandKey);location.hash=`board/${result.id}`;
    }
    if(form.dataset.form==='board-cancel')result=await api(`/api/board/tasks/${encodeURIComponent(form.dataset.id)}/cancel`,{});
    if(form.dataset.form==='board-review') {
      if(values.decision==='changes_requested'&&!values.feedback.trim())throw new Error('退回修改时请填写评审意见。');
      form.dataset.commandKey ||= crypto.randomUUID();
      result=await api(`/api/board/tasks/${encodeURIComponent(form.dataset.id)}/review`,{run_id:form.dataset.run,decision:values.decision,feedback:values.feedback},'POST',form.dataset.commandKey);
    }
    if(form.dataset.form==='message-send') {
      if(new TextEncoder().encode(values.text).length>8000)throw new Error('消息超过 8,000 个 UTF-8 字节。');
      const input={text:values.text};for(const field of ['correlation_id','causation_id'])if(values[field])input[field]=values[field];
      result=await api(`/api/v1/agent-instances/${encodeURIComponent(values.recipient_id)}/messages`,input,'POST',form.dataset.commandKey);location.hash='researcher';
    }
    if(form.dataset.form==='message-action') {
      if(new TextEncoder().encode(values.note).length>2000)throw new Error('处置说明超过 2,000 个 UTF-8 字节。');
      result=await api(`/api/v1/messages/${encodeURIComponent(form.dataset.id)}/actions`,{action:form.dataset.operation,note:values.note},'POST',form.dataset.commandKey);
    }
    if(form.dataset.form==='drain') {result=await api(`/api/v1/agent-instances/${encodeURIComponent(form.dataset.id)}/actions`,{action:'drain'});}
    if(form.dataset.form==='terminate') {result=await api(`/api/v1/agent-instances/${encodeURIComponent(form.dataset.id)}/actions`,{action:'terminate'});}
    if(form.dataset.form==='spawn') {const preset=JSON.parse(values.preset);result=await api('/api/v1/agent-fleets',{preset_name:preset.name,preset_version:preset.version,count:Number(values.count),mode:values.mode,parameters:instanceParameters(form),ttl_seconds:values.ttl_minutes?Number(values.ttl_minutes)*60:null},'POST',form.dataset.commandKey);location.hash=`fleet/${result.fleet_id}`;researcherSections.add('fleets');}
    if(form.dataset.form==='fleet-action') {result=await api(`/api/v1/agent-fleets/${encodeURIComponent(form.dataset.id)}/actions`,{action:form.dataset.operation,expected_revision:Number(form.dataset.revision),...(form.dataset.operation==='scale'?{count:Number(values.count)}:{})},'POST',form.dataset.commandKey);}
    if(form.dataset.form==='preset') {result=await api('/api/v1/presets',presetPayload(form));location.hash='presets';}
    if(form.dataset.form==='process-state') {result=await api(`/api/v1/process-plugins/${encodeURIComponent(form.dataset.id)}/${encodeURIComponent(form.dataset.version)}/actions`,{action:form.dataset.state});}
    if(form.dataset.form==='run') {result=await api(`/api/v1/agent-instances/${encodeURIComponent(form.dataset.id)}/runs`,{text:values.text},'POST',form.dataset.commandKey);location.hash='researcher';researcherSections.add('runs');}
    $('#modal').close();await refresh(true);
    if(form.dataset.form==='run-cancel'){await runDetail(form.dataset.id);toast('取消请求已提交，运行将协作收尾。');return;}
    if(form.dataset.form==='board-publish'){toast('任务已发布，群体会自行安排合适的成员。');return;}
    if(form.dataset.form==='board-cancel'){toast('取消请求已保存。');return;}
    if(form.dataset.form==='board-review'){toast(result.review_state==='accepted'?'评审通过，成果已交付归档。':'评审意见已保存，成员将按意见重新开工。');return;}
    if(form.dataset.form==='run'&&data.runs.some(r=>r.id===result.id)){await runDetail(result.id);}
    toast(form.dataset.form==='message-send'?'消息已接纳；投递与消费状态见研究者视图。':form.dataset.form==='message-action'?'处置记录已保存。':form.dataset.form==='process-state'?(result.state==='disabled'?'插件版本已禁用，新的调用将被拒绝。':'健康检查通过，插件版本已启用。'):result?.operation_id ? '操作已接纳，协调进度见Agent 池详情。' : result?.count ? `已创建 ${result.count} 个独立实例` : form.dataset.form==='run'?'任务已接纳，可在执行详情中检查结果。':form.dataset.form==='drain'?'排空请求已保存，活动任务结算后终止。':form.dataset.form==='terminate'?'实例已终止，历史记录已保留。':'版本已保存');
  }catch(error){const target=form.querySelector('#form-error');if(target){target.textContent=error.message;target.tabIndex=-1;target.focus();}}
  finally {submit.disabled=false;submit.textContent=label;form.removeAttribute('aria-busy');}
});
document.querySelectorAll('nav a').forEach(a=>{a.title=a.textContent.replace(/[◈▦▧▷⌘◫≋◇0]/g,'').trim();a.querySelector('.nav-icon').innerHTML=icon(a.dataset.page);});
document.addEventListener('compositionstart',()=>{composing=true;});
document.addEventListener('compositionend',()=>{composing=false;});
document.addEventListener('input',event=>{
  const type=event.target.dataset.filter,kind=page()==='researcher'?(researcherSections.has('agents')?'agents':'runs'):page();
  if(!type||!filters[kind])return;
  filters[kind][type]=event.target.value;
  $('#directory-results').innerHTML=directory(kind);
  $('.filter-note').textContent=resultCount(kind);
  const clear=$('.toolbar [data-action="clear-filters"]');
  if(clear)clear.hidden=!filters[kind].query&&filters[kind].state==='all';
  else if(filters[kind].query||filters[kind].state!=='all')$('.toolbar').insertAdjacentHTML('beforeend',button('清除筛选','clear-filters'));
});
$('#modal').addEventListener('close',()=>{
  if($('#modal').open)return;
  inspectionRequest++;
  if(modalTrigger?.isConnected)modalTrigger.focus({preventScroll:true});
  else if(modalTrigger?.dataset.action){const match=[...document.querySelectorAll('[data-action]')].find(el=>el.dataset.action===modalTrigger.dataset.action&&el.dataset.id===modalTrigger.dataset.id);match?.focus({preventScroll:true});}
});
$('#modal').addEventListener('cancel',event=>{if($('#modal form[aria-busy="true"]'))event.preventDefault();});
$('#login-dialog').addEventListener('cancel',event=>event.preventDefault());
$('.skip-link').addEventListener('click',event=>{event.preventDefault();$('#content').focus();});
function closeNavigation(){ $('#shell').classList.remove('navigation-open');$('#navigation-toggle').setAttribute('aria-expanded','false'); }
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&$('#shell').classList.contains('navigation-open')){closeNavigation();$('#navigation-toggle').focus();}});
window.addEventListener('hashchange',()=>{applyPageRedirect();inspectionRequest++;if($('#modal').open&&!$('#modal form[aria-busy="true"]'))$('#modal').close();closeNavigation();render();if(boardId()||fleetId())refresh(true);$('#content').focus({preventScroll:true});window.scrollTo({top:0,behavior:'instant'});});
applyPageRedirect();
refresh();setInterval(()=>{if(authenticated&&!document.hidden)refresh();},1500);
window.addEventListener('focus',()=>{if(authenticated)refresh();});
