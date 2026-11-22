'use strict';

const boardStates={staged:'等待前置工作完成',pending:'等待接活',running:'进行中',waiting:'子任务进行中',completed:'已完成',failed:'未完成',cancelled:'已取消',blocked:'上游受阻',interrupted:'结果未知'};
const boardTerminal=new Set(['completed','failed','cancelled','blocked','interrupted']);
let selectedBoard=null,boardTraces=null,boardEnergy=null,boardNotes=null;
const boardTraceKinds={result:'结果',finding:'发现',path:'路径',blocker:'阻断'};
function boardId(){const parts=(location.pathname||'/').split('/').filter(Boolean);return parts[0]==='board'&&parts[1]?parts[1]:null;}
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
  }
  catch(error){if(boardId()===id)selectedBoard={id,loadError:error.message};}
}
function boardData(snapshot=data){return snapshot.board||{tasks:[],agents:[]};}
function boardRoute(id){return (boardData().routing||[]).find(route=>route.task_id===id);}
function boardRoutingMessage(code){return ({board_no_suitable_preset:'暂时没有擅长这类工作的成员',board_no_eligible_preset:'暂无可用成员，请检查 Preset、模型和插件配置',decision_low_confidence:'还在斟酌由谁来做',board_worker_capacity:'成员都在忙，排队等待',instance_quota_exceeded:'成员数量已达上限，等待空位',decision_network_error:'安排工作的服务连接失败',decision_invalid_response:'安排工作的服务返回了无效结果'})[code]||'安排暂时受阻，稍后自动重试';}
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
  return `<a class="board-task-row" href="/board/${esc(task.id)}"><span class="board-task-copy"><strong>${esc(task.title)}</strong>${task.instructions?`<span class="board-task-preview">${esc(task.instructions)}</span>`:''}<span>${esc(boardPreset(task)?`由 ${task.preset_name} 负责`:route?.error?boardRoutingMessage(route.error):'群体自行安排合适的成员')}${task.parent_id?' 子任务':''}</span></span>${boardReuseBadge(task)}${badge(boardBadgeClass(task),boardStatus(task,route))}<span aria-hidden="true">→</span></a>`;
}
// —— 研究者折叠区：机制参数与原始记录 ——




let boardTab='current';
function boardPage(){
  const board=boardData(),tasks=board.tasks||[],id=boardId();
  if(id){
    if(selectedBoard?.id===id&&selectedBoard.loadError)return `<a class="back-link" href="/board">← 任务板</a><div class="callout" role="alert">${esc(selectedBoard.loadError)} ${button('重试','board-refresh')}</div>`;
    const task=selectedBoard?.id===id?selectedBoard:tasks.find(t=>t.id===id);
    return task?boardDetail(task,tasks):`<a class="back-link" href="/board">← 任务板</a>${empty('任务未出现在当前快照','请返回任务板或刷新后重试。',button('刷新','board-refresh'))}`;
  }
  const roots=tasks.filter(t=>!t.parent_id),active=tasks.filter(t=>!boardTerminal.has(t.state));
  const history=roots.filter(boardArchived),current=roots.filter(t=>!boardArchived(t));
  const reviewCount=roots.filter(t=>t.review_state==='pending').length;
  const currentBody=(current.length?`<div class="board-task-list">${current.map(boardTaskRow).join('')}</div>`:empty(roots.length?'当前没有待处理任务':'还没有发布任务',roots.length?'历史任务保留在历史分页中。发布新任务后，在这里跟进进展与结果。':'点击右上角“发布任务”，填写目标与验收标准。'))+
    `<p class="hint board-summary">${active.length} 项待完成，含子任务${reviewCount?`，${reviewCount} 项等你验收`:''}</p>`;
  const historyBody=history.length?`<div class="board-task-list">${history.map(boardTaskRow).join('')}</div>`:empty('还没有历史任务','任务交付或取消后会归档在这里。');
  return head('任务板', button('发布任务','board-publish',true))+
    (board.router?.error?`<div class="callout" role="status">安排工作暂时受阻：${esc(board.router.error)}</div>`:'')+
    tabBar('board-tab',[['current',`当前任务 ${current.length}`],['history',`历史任务 ${history.length}`]],boardTab)+
    `<div class="page-tab">${tabPanel(boardTab==='history'?historyBody:currentBody,true)}</div>`+
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
  return `<a class="back-link" href="/board">← 任务板</a>`+head(esc(task.title), !boardTerminal.has(task.state)?button('取消任务','board-cancel',false,`data-id="${esc(task.id)}"`):'')+
    `<div class="goal-summary"><div><span>进度</span>${badge(boardBadgeClass(task),boardStatus(task,route))}</div><div><span>由谁负责</span><strong>${esc(boardPreset(task)?task.preset_name:'群体安排中')}</strong></div><div><span>子任务</span><strong>${children.filter(t=>t.state==='completed').length} / ${children.length}</strong></div>${boardEnergySummary(task)}</div>`+
    (!task.parent_id && task.energy && !boardTerminal.has(task.state)?`<div class="inspector-actions">${task.energy.available<=0?'预算耗尽，等待补充能量。':''}${button('补充能量','energy-top-up',false,`data-id="${esc(task.id)}"`)}</div>`:'')+
    (reuse?`<p class="hint">这份工作的成果已被同伴参考 ${reuse} 次。</p>`:'')+
    (routingError?`<div class="callout" role="status">${esc(boardRoutingMessage(route.error))}${route.retry_at?`。下次尝试 ${esc(time(route.retry_at))}`:''}${route.attempts?`，已尝试 ${esc(route.attempts)} 次`:''}。</div>`:'')+
    (task.error?`<div class="callout" role="status">${task.error==='energy_exhausted'?'预算耗尽，工作已停止。可取消后以更高预算重新发布。':task.error==='board_run_failed'?'这次执行没有完成，工作已重新开放给群体。':task.error==='interrupted_outcome_unknown'?'执行结果无法确认，请核对后再决定。':task.error==='board_agent_did_not_claim'?'成员没有接下这项工作，已重新开放。':esc('这次工作未能完成，已记录以便排查。')}</div>`:'')+
    (task.progress?panel('工作进展',`<div class="panel-body record-text">${esc(task.progress.message)}</div>`):'')+
    panel('任务要求',`<div class="panel-body"><dl class="definition"><dt>目标</dt><dd>${esc(task.instructions)}</dd><dt>验收标准</dt><dd>${esc(task.acceptance)}</dd>${parent?`<dt>拆分来源</dt><dd><a href="/board/${esc(parent.id)}">${esc(parent.title)}</a></dd>`:''}</dl></div>`)+
    (children.length?panel('拆分后的任务',`<div class="board-task-list">${children.map(boardTaskRow).join('')}</div>`):'')+
    (result?panel(task.result?'成果':'上轮成果（待修改）',`<div class="panel-body output">${typeof result==='string'?esc(result):dataView(result)}</div>`):'')+
    boardAuditPanel(task)+
    boardNotesPanel(task)+
    (task.review_state==='pending'&&!boardAuditing(task)?panel('等你验收',`<div class="panel-body"><p>根据验收标准检查成果。通过后即交付归档；退回修改将按你的意见重新开工。</p>${button('评审成果','board-review',true,`data-id="${esc(task.id)}" data-run="${esc(task.run_id)}"`)}</div>`):'')+
    (task.reviews?.length?panel('最近评审记录',`<div class="panel-body">${task.reviews.map(review=>`<div class="record-text"><strong>${review.decision==='accepted'?'通过':'退回修改'}</strong>${review.author_kind==='verifier'?' <span class="hint">（独立质检）</span>':' <span class="hint">（人类）</span>'}<p>${esc(review.feedback)}</p>${button('查看本轮执行记录','run-detail',false,`data-id="${esc(review.run_id)}"`)}</div>`).join('')}</div>`):'')+
    `<div class="head-actions">${task.instance_id?button('负责成员','agent-detail',false,`data-id="${esc(task.instance_id)}"`):''}${task.run_id?button('执行记录','run-detail',false,`data-id="${esc(task.run_id)}"`):''}<a class="button" href="/studio">工作现场</a></div>`;
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
  openModal('发布任务',`<form data-form="board-publish"><label>任务名称<input name="title" required maxlength="200" placeholder="例如：分析这个项目的性能瓶颈"></label><label>任务内容<textarea name="instructions" required maxlength="8000" rows="5" placeholder="描述目标、已有资料和约束。"></textarea></label><label>验收标准<textarea name="acceptance" required maxlength="8000" rows="3" placeholder="完成时应交付什么，如何判断结果可用？"></textarea></label><label>能量预算（可选）<input name="energy_budget" type="number" min="1" max="1000000000000" step="1" placeholder="默认 1 亿，耗尽可补充"></label><p class="form-section-title">独立验证（可选）</p><label>验证器 Preset<select name="verify_preset"><option value="">不启用（仅人类评审）</option>${(data?.presets||[]).map(p=>`<option value="${esc(p.name)}@${esc(p.version)}">${esc(p.name)} @ ${esc(p.version)}</option>`).join('')}</select></label><label>自动返工上限<select name="verify_reworks"><option value="0">0（退回即交人类）</option><option value="1" selected>1</option><option value="2">2</option></select></label><p class="hint">全新实例按验收标准审计冻结结果；通过自动归档，退回有界返工，人类保留最终评审权。</p>${formEnd('投递到任务板')}</form>`,'共享任务板');
}
async function boardAction(action,element){
  if(action==='board-publish'){boardPublishForm();return true;}
  if(action==='board-refresh'){await refresh(true);return true;}
  if(action==='energy-top-up'){
    openModal('补充任务能量',`<form data-form="energy-top-up" data-id="${esc(element.dataset.id)}"><label>增加能量<input name="amount" type="number" min="1" max="1000000000000" step="1" required></label>${formEnd('补充能量')}</form>`);
    return true;
  }
  if(action==='board-review'){
    openModal('评审任务成果',`<form data-form="board-review" data-id="${esc(element.dataset.id)}" data-run="${esc(element.dataset.run)}"><label>评审决定<select name="decision"><option value="changes_requested">退回修改</option><option value="accepted">通过</option></select></label><label>评审意见<textarea name="feedback" maxlength="8000" rows="5" placeholder="退回时请说明需要修改什么；通过时可留空。"></textarea></label><p>退回修改将重新开工，可能产生模型调用费用。</p>${formEnd('提交评审')}</form>`,'人类评审');return true;
  }
  if(action==='board-cancel'){
    openModal('取消这项任务？',`<form data-form="board-cancel" data-id="${esc(element.dataset.id)}"><p>取消后停止后续接活，并请求正在执行的实例收尾。已产生的工具效果和历史记录保留。</p>${formEnd('确认取消')}</form>`,'任务生命周期');
    $('#modal').setAttribute('role','alertdialog');return true;
  }
  return false;
}