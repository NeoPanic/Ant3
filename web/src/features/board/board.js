'use strict';

const boardStates={staged:'等待前置工作完成',pending:'等待接活',running:'进行中',waiting:'子任务进行中',completed:'已完成',failed:'未完成',cancelled:'已取消',blocked:'上游受阻',interrupted:'结果未知'};
const boardTerminal=new Set(['completed','failed','cancelled','blocked','interrupted']);
let selectedBoard=null,boardTraces=null,boardEnergy=null,boardNotes=null;
const boardTraceKinds={result:'结果',finding:'发现',path:'路径',blocker:'阻断'};
function boardId(){return location.hash.startsWith('#board/')?location.hash.slice(7):null;}
function boardTraceStrength(trace){const value=Number(trace?.strength);return Number.isFinite(value)?Math.max(0,Math.min(100,value)):0;}
// 浓度徽章只在研究者折叠区出现。
function boardTraceBadge(trace){
  if(!trace)return '';
  const strength=boardTraceStrength(trace),tier=strength>=66?'strong':strength>=33?'medium':'faint';
  return `<span class="badge trace-badge ${tier}" title="信息素痕迹 ${esc(trace.kind)} · 浓度 ${strength.toFixed(1)} / 100"><span class="trace-bar" aria-hidden="true"><i style="width:${strength.toFixed(1)}%"></i></span>${esc(boardTraceKinds[trace.kind]||trace.kind)} ${strength.toFixed(1)}</span>`;
}
// 列表行的人话徽章：只讲成果被参考的行为，不显示机制数值。
function boardReuseBadge(task){
  const count=Number(task?.reuse_count)||0;
  return count>0?badge('neutral',`被参考 ${count} 次`):'';
}
function boardRootId(task,tasks){
  if(task.root_id)return task.root_id;
  let current=task;const seen=new Set([task.id]);
  while(current?.parent_id&&!seen.has(current.parent_id)){seen.add(current.parent_id);current=(tasks||[]).find(t=>t.id===current.parent_id);}
  return current?.id||task.id;
}
function boardTraceHalfLife(ms){
  const value=Number(ms);if(!Number.isFinite(value)||value<=0)return '未提供';
  if(value>=3600000)return `${(value/3600000).toFixed(1)} 小时`;
  if(value>=60000)return `${(value/60000).toFixed(1)} 分钟`;
  return `${(value/1000).toFixed(1)} 秒`;
}
async function boardReadDetail(){
  const id=boardId();if(!id)return;
  try{
    const task=await api(`/api/board/tasks/${encodeURIComponent(id)}`);
    if(boardId()!==id)return;
    selectedBoard=task;
    const rootId=boardRootId(task,boardData().tasks||[]);
    try{const field=await api(`/api/board/traces?root_id=${encodeURIComponent(rootId)}`);if(boardId()===id)boardTraces=field;}
    catch(error){if(boardId()===id)boardTraces={root_id:rootId,loadError:error.message};}
    try{const energy=await api(`/api/board/tasks/${encodeURIComponent(id)}/energy`);if(boardId()===id)boardEnergy={id,...energy};}
    catch{if(boardId()===id)boardEnergy=null;}
    try{const notes=await api(`/api/board/tasks/${encodeURIComponent(id)}/notes`);if(boardId()===id)boardNotes={id,...notes};}
    catch{if(boardId()===id)boardNotes=null;}
  }
  catch(error){if(boardId()===id){selectedBoard={id,loadError:error.message};boardTraces=null;boardEnergy=null;boardNotes=null;}}
}
function boardData(snapshot=data){return snapshot.board||{tasks:[],agents:[]};}
function boardRoute(id){return (boardData().routing||[]).find(route=>route.task_id===id);}
function boardRoutingMessage(code){return ({board_no_suitable_preset:'暂时没有擅长这类工作的成员',board_no_eligible_preset:'暂无可用成员，请检查母体、模型和插件配置',decision_low_confidence:'还在斟酌由谁来做',board_worker_capacity:'成员都在忙，排队等待',instance_quota_exceeded:'成员数量已达上限，等待空位',decision_network_error:'安排工作的服务连接失败',decision_invalid_response:'安排工作的服务返回了无效结果'})[code]||'安排暂时受阻，稍后自动重试';}
function boardProvisioned(task){return task.route_state?task.route_state==='provisioned':Boolean(task.instance_id||task.preset_name);}
function boardPreset(task){return task.state==='pending'&&!boardProvisioned(task)?null:task.preset_name;}
function boardAuditing(task){const a=task.audits?.[task.audits.length-1];return a&&['pending','running'].includes(a.state)?a:null;}
function boardBadgeClass(task){
  if(boardAuditing(task))return 'running';
  if(task.state==='completed')return task.review_state==='accepted'?'accepted':'awaiting-review';
  if(task.review_state==='changes_requested'&&task.state!=='cancelled')return 'changes_requested';
  if(task.state==='failed')return 'failed';
  if(['running','waiting'].includes(task.state))return 'running';
  if(task.state==='cancelled')return 'cancelled';
  return 'queued';
}
function boardStatus(task,route){
  if(boardAuditing(task))return '独立质检中';
  if(task.state==='completed')return task.review_state==='accepted'?'已交付':'等你验收';
  if(task.review_state==='changes_requested'&&task.state!=='cancelled')return '按意见修改中';
  if(task.state==='pending')return boardProvisioned(task)?'等待开工':route?.state==='routing'?'正在找合适的成员':route?.error?'安排受阻，稍后重试':'等待接活';
  if(task.state==='failed'&&task.error==='energy_exhausted')return '预算耗尽';
  return boardStates[task.state]||task.state;
}
function boardArchived(task){return task.state==='cancelled'||(task.state==='completed'&&task.review_state==='accepted');}
function boardTaskRow(task){
  const route=boardRoute(task.id);
  return `<a class="board-task-row" href="#board/${esc(task.id)}"><span class="board-task-copy"><strong>${esc(task.title)}</strong>${task.instructions?`<span class="board-task-preview">${esc(task.instructions)}</span>`:''}<span>${esc(boardPreset(task)?`由 ${task.preset_name} 负责`:route?.error?boardRoutingMessage(route.error):'群体自行安排合适的成员')}${task.parent_id?' 子任务':''}</span></span>${boardReuseBadge(task)}${badge(boardBadgeClass(task),boardStatus(task,route))}<span aria-hidden="true">→</span></a>`;
}
// —— 研究者折叠区：机制参数与原始记录 ——
function boardTraceFieldPanel(task,tasks){
  const rootId=boardRootId(task,tasks),field=boardTraces;
  const refreshButton=button('刷新','board-refresh');
  let body;
  if(field?.root_id===rootId&&field.loadError)body=`<div class="panel-body"><p class="hint">痕迹场读取失败：${esc(field.loadError)}</p></div>`;
  else if(field?.root_id!==rootId)body=`<div class="panel-body"><p class="hint">正在读取痕迹场…</p></div>`;
  else{
    const traces=[...(field.traces||[])].sort((a,b)=>boardTraceStrength(b)-boardTraceStrength(a));
    body=traces.length?table(['痕迹键','类型','浓度','沉积次数'],traces.map(trace=>`<tr><td class="mono">${esc(trace.key)}</td><td><span class="badge neutral">${esc(boardTraceKinds[trace.kind]||trace.kind)}</span></td><td><span class="trace-cell">${boardTraceBadge(trace)}<span class="mono">/ ${esc(field.max_strength??100)}</span></span></td><td class="mono">${esc(trace.deposits??0)}</td></tr>`)):`<div class="panel-body"><p class="hint">该根任务还没有信息素痕迹。Agent 结算成果后会在此沉积。</p></div>`;
  }
  const halfLife=field?.root_id===rootId&&!field.loadError?`<p class="hint trace-half-life">信息素随时间蒸发：半衰期 ${boardTraceHalfLife(field.half_life_ms)}${Number.isFinite(Number(field.half_life_ms))?`（half_life_ms = ${esc(field.half_life_ms)}）`:''}，即每隔该时长浓度减半；新的沉积会重新加浓。列表按当前浓度降序，随页面每 1.5 秒轮询自动更新。</p>`:'';
  return panel('信息素痕迹场',body+halfLife,refreshButton,`<small>根任务 <span class="mono">${esc(rootId)}</span></small>`);
}
function boardEnergyLedgerPanel(task){
  const ledger=boardEnergy?.id===task.id?boardEnergy:null;
  if(!ledger)return '<p class="hint">正在读取能量账本…</p>';
  const rows=ledger.ledger||[];
  const kinds={reserve:'预留',settle:'结算消耗',settle_unknown:'结果未知结算',accept_refund:'验收回充',complete_refund:'完成回充',refund:'回充',idle_burn:'待命消耗'};
  return `<dl class="detail-kv"><dt>预算 / 预留</dt><dd class="mono">${esc(ledger.budget)} / ${esc(ledger.reserved)}</dd><dt>已耗 / 回充 / 可用</dt><dd class="mono">${esc(ledger.spent)} / ${esc(ledger.refunded)} / ${esc(ledger.available)}</dd></dl>`+
    (rows.length?table(['运行','类型','数量','结余','时间'],rows.map(entry=>`<tr><td class="mono">${esc(short(entry.run_id||'—'))}</td><td>${esc(kinds[entry.kind]||entry.kind)}</td><td class="mono">${esc(entry.amount)}</td><td class="mono">${esc(entry.balance)}</td><td>${time(entry.created_at)}</td></tr>`)):'<p class="hint">尚无记账记录。</p>');
}
function boardNotesHistoryPanel(task){
  const source=boardNotes?.id===task.id?boardNotes.notes:task.notes;
  if(!source)return '<p class="hint">正在读取黑板裁决历史…</p>';
  if(!source.length)return '<p class="hint">尚无黑板记录。</p>';
  const status={active:'现行结论',superseded:'被裁决取代'};
  return table(['键','修订','状态','支持度','内容'],source.map(n=>`<tr><td class="mono wrap">${esc(n.key)}</td><td class="mono">r${esc(n.revision)}</td><td>${esc(status[n.status]||n.status)}${n.superseded_by?`<div class="secondary-text mono">${esc(short(n.superseded_by))}</div>`:''}</td><td class="mono">${esc(n.support_score??n.support??'—')}</td><td class="wrap record-text">${esc(n.body)}</td></tr>`));
}
function boardResearcherDetails(task,tasks){
  return `<details class="researcher" id="board-researcher"><summary>研究者视图：协作机制与原始记录</summary><div class="researcher-body">
    ${boardTraceFieldPanel(task,tasks)}
    ${panel('能量账本',`<div class="panel-body">${boardEnergyLedgerPanel(task)}</div>`)}
    ${panel('黑板裁决历史',`<div class="panel-body">${boardNotesHistoryPanel(task)}</div>`)}
    ${task.route_reason?`<details class="raw-data"><summary>选型依据原文（route_reason）</summary><p class="record-text">${esc(task.route_reason)}</p></details>`:''}
    ${task.trace?`<p class="hint">当前痕迹：${esc(boardTraceKinds[task.trace.kind]||task.trace.kind)} · 浓度 ${boardTraceStrength(task.trace).toFixed(1)} · 沉积 ${esc(task.trace.deposits??0)} 次</p>`:''}
  </div></details>`;
}
function boardPage(){
  const board=boardData(),tasks=board.tasks||[],id=boardId();
  if(id){
    if(selectedBoard?.id===id&&selectedBoard.loadError)return `<a class="back-link" href="#board">← 任务板</a><div class="callout" role="alert">${esc(selectedBoard.loadError)} ${button('重试','board-refresh')}</div>`;
    const task=selectedBoard?.id===id?selectedBoard:tasks.find(t=>t.id===id);
    return task?boardDetail(task,tasks):`<a class="back-link" href="#board">← 任务板</a>${empty('任务未出现在当前快照','请返回任务板或刷新后重试。',button('刷新','board-refresh'))}`;
  }
  const roots=tasks.filter(t=>!t.parent_id),active=tasks.filter(t=>!boardTerminal.has(t.state));
  const history=roots.filter(boardArchived),current=roots.filter(t=>!boardArchived(t));
  const reviewCount=roots.filter(t=>t.review_state==='pending').length;
  return head('任务板', button('发布任务','board-publish',true))+
    (board.router?.error?`<div class="callout" role="status">安排工作暂时受阻：${esc(board.router.error)}</div>`:'')+
    panel('当前任务',current.length?`<div class="board-task-list">${current.map(boardTaskRow).join('')}</div>`:empty(roots.length?'当前没有待处理任务':'还没有发布任务',roots.length?'历史任务保留在下方。发布新任务后，在这里跟进进展与结果。':'点击右上角“发布任务”，填写目标与验收标准。'),`<span class="board-count">${active.length} 项待完成，含子任务${reviewCount?`，${reviewCount} 项等你验收`:''}</span>`)+
    (history.length?`<details id="board-history" class="board-history"><summary>历史任务 <span>${history.length} 项已交付或取消</span></summary><div class="board-task-list">${history.map(boardTaskRow).join('')}</div></details>`:'')+
    (board.history_truncated?`<p class="hint">显示最近 ${esc(board.roots_included)} 项根任务及其子任务，共 ${esc(board.roots_total)} 项根任务。活动任务优先。</p>`:'');
}
function boardEnergySummary(task){
  if(!task.energy)return '';
  const budget=Number(task.energy.budget)||0,available=Number(task.energy.available)||0;
  const used=budget?Math.max(0,Math.min(100,(budget-available)/budget*100)):0;
  return `<div><span>花费</span><strong title="已耗 ${esc(task.energy.spent)} · 验收回充 ${esc(task.energy.refunded)}"><span class="progress-track energy-inline ${used>=80?'is-low':used>=50?'is-mid':''}" role="img" aria-label="预算已用 ${Math.round(used)}%"><i style="width:${used.toFixed(1)}%"></i></span><small class="hint"> 还剩 ${available.toLocaleString()} / ${budget.toLocaleString()}</small></strong></div>`;
}
function boardDetail(task,tasks){
  const children=tasks.filter(t=>t.parent_id===task.id),parent=tasks.find(t=>t.id===task.parent_id);
  const route=boardRoute(task.id),routingError=task.state==='pending'&&route?.error;
  const report=task.result||task.previous_result,result=typeof report==='string'?report:report?.output||report;
  const reuse=Number(task.reuse_count)||0;
  return `<a class="back-link" href="#board">← 任务板</a>`+head(esc(task.title), !boardTerminal.has(task.state)?button('取消任务','board-cancel',false,`data-id="${esc(task.id)}"`):'')+
    `<div class="goal-summary"><div><span>进度</span>${badge(boardBadgeClass(task),boardStatus(task,route))}</div><div><span>由谁负责</span><strong>${esc(boardPreset(task)?task.preset_name:'群体安排中')}</strong></div><div><span>子任务</span><strong>${children.filter(t=>t.state==='completed').length} / ${children.length}</strong></div>${boardEnergySummary(task)}</div>`+
    (reuse?`<p class="hint">这份工作的成果已被同伴参考 ${reuse} 次。</p>`:'')+
    (routingError?`<div class="callout" role="status">${esc(boardRoutingMessage(route.error))}${route.retry_at?`。下次尝试 ${esc(time(route.retry_at))}`:''}${route.attempts?`，已尝试 ${esc(route.attempts)} 次`:''}。</div>`:'')+
    (task.error?`<div class="callout" role="status">${task.error==='energy_exhausted'?'预算耗尽，工作已停止。可取消后以更高预算重新发布。':esc(task.error)}</div>`:'')+
    (task.progress?panel('工作进展',`<div class="panel-body record-text">${esc(task.progress.message)}</div>`):'')+
    panel('任务要求',`<div class="panel-body"><dl class="definition"><dt>目标</dt><dd>${esc(task.instructions)}</dd><dt>验收标准</dt><dd>${esc(task.acceptance)}</dd>${parent?`<dt>拆分来源</dt><dd><a href="#board/${esc(parent.id)}">${esc(parent.title)}</a></dd>`:''}</dl></div>`)+
    (children.length?panel('拆分后的任务',`<div class="board-task-list">${children.map(boardTaskRow).join('')}</div>`):'')+
    (result?panel(task.result?'成果':'上轮成果（待修改）',`<div class="panel-body output">${typeof result==='string'?esc(result):dataView(result)}</div>`):'')+
    boardAuditPanel(task)+
    boardNotesPanel(task)+
    (task.review_state==='pending'&&!boardAuditing(task)?panel('等你验收',`<div class="panel-body"><p>根据验收标准检查成果。通过后即交付归档；退回修改将按你的意见重新开工。</p>${button('评审成果','board-review',true,`data-id="${esc(task.id)}" data-run="${esc(task.run_id)}"`)}</div>`):'')+
    (task.reviews?.length?panel('最近评审记录',`<div class="panel-body">${task.reviews.map(review=>`<div class="record-text"><strong>${review.decision==='accepted'?'通过':'退回修改'}</strong>${review.author_kind==='verifier'?' <span class="hint">（独立质检）</span>':' <span class="hint">（人类）</span>'}<p>${esc(review.feedback)}</p>${button('查看本轮执行记录','run-detail',false,`data-id="${esc(review.run_id)}"`)}</div>`).join('')}</div>`):'')+
    `<div class="head-actions">${task.instance_id?button('负责成员','agent-detail',false,`data-id="${esc(task.instance_id)}"`):''}${task.run_id?button('执行记录','run-detail',false,`data-id="${esc(task.run_id)}"`):''}<a class="button" href="#studio">工作现场</a></div>`+
    boardResearcherDetails(task,tasks);
}
function boardNotesPanel(task){
  if(task.parent_id)return '';
  const notes=task.notes||[];
  if(!notes.length)return '';
  const active=notes.filter(n=>n.status==='active');
  if(!active.length)return '';
  const rows=active.map(n=>`<div class="record-text"><strong>${esc(n.key)}</strong><p>${esc(n.body)}</p></div>`).join('');
  return panel('工作共识',`<div class="panel-body">${rows}<p class="hint">成员共享的现行结论；出现分歧时，证据更充分的一方胜出。内容为成员提供的证据，请自行核对。</p></div>`);
}
const auditStates={pending:'等待质检',running:'质检中',passed:'质检通过','changes-requested':'质检退回',inconclusive:'证据不足',failed:'质检未能完成',cancelled:'已取消',stale:'成果已更新'};
function boardAuditPanel(task){
  if(!task.verification&&!task.audits?.length)return '';
  const audits=task.audits||[];
  const rows=audits.map(a=>`<div class="record-text"><strong>第 ${esc(a.attempt)} 次 · ${esc(auditStates[a.state]||a.state)}</strong>${a.verdict?`<p>${esc(a.verdict.status==='passed'?'通过':a.verdict.status==='changes-requested'?'退回修改':a.verdict.status)}：${esc(a.verdict.reason)}</p>`:''}${a.error?`<p class="hint">${esc(a.error)}</p>`:''}${a.run_id?button('质检执行记录','run-detail',false,`data-id="${esc(a.run_id)}"`):''}</div>`).join('');
  const note=task.verification?'<p class="hint">这项工作启用了独立质检：由一位全新成员按验收标准检查成果。通过即交付；退回会附修改意见并有限返工；证据不足时交你定夺。</p>':'';
  return panel('质检结论',`<div class="panel-body">${note}${rows||'<p class="hint">成果完成后自动开始质检。</p>'}</div>`);
}
function boardPublishForm(){
  openModal('发布任务',`<form data-form="board-publish"><label>任务名称<input name="title" required maxlength="200" placeholder="例如：分析这个项目的性能瓶颈"></label><label>任务内容<textarea name="instructions" required maxlength="8000" rows="5" placeholder="描述目标、已有资料和约束。"></textarea></label><label>验收标准<textarea name="acceptance" required maxlength="8000" rows="3" placeholder="完成时应交付什么，如何判断结果可用？"></textarea></label><label>能量预算（可选）<input name="energy_budget" type="number" min="2000" max="10000000" step="1000" placeholder="默认 100000；验收通过会部分回充，耗尽即停止"></label><details><summary>独立验证（可选）</summary><label>验证器母体<select name="verify_preset"><option value="">不启用（仅人类评审）</option>${(data?.presets||[]).map(p=>`<option value="${esc(p.name)}@${esc(p.version)}">${esc(p.name)} @ ${esc(p.version)}</option>`).join('')}</select></label><label>自动返工上限<select name="verify_reworks"><option value="0">0（退回即交人类）</option><option value="1" selected>1</option><option value="2">2</option></select></label><p class="hint">全新实例按验收标准审计冻结结果；通过自动归档，退回有界返工，人类保留最终评审权。</p></details>${formEnd('投递到任务板')}</form>`,'共享任务板');
}
async function boardAction(action,element){
  if(action==='board-publish'){boardPublishForm();return true;}
  if(action==='board-refresh'){await refresh(true);return true;}
  if(action==='board-review'){
    openModal('评审任务成果',`<form data-form="board-review" data-id="${esc(element.dataset.id)}" data-run="${esc(element.dataset.run)}"><label>评审决定<select name="decision"><option value="changes_requested">退回修改</option><option value="accepted">通过</option></select></label><label>评审意见<textarea name="feedback" maxlength="8000" rows="5" placeholder="退回时请说明需要修改什么；通过时可留空。"></textarea></label><p>退回修改将重新开工，可能产生模型调用费用。</p>${formEnd('提交评审')}</form>`,'人类评审');return true;
  }
  if(action==='board-cancel'){
    openModal('取消这项任务？',`<form data-form="board-cancel" data-id="${esc(element.dataset.id)}"><p>取消后停止后续接活，并请求正在执行的实例收尾。已产生的工具效果和历史记录保留。</p>${formEnd('确认取消')}</form>`,'任务生命周期');
    $('#modal').setAttribute('role','alertdialog');return true;
  }
  return false;
}