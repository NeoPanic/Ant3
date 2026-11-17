'use strict';

// 研究者视图：机制参数与原始记录的聚合入口。旧页面函数全部保留，仅收起展示。
// 分节惰性渲染：只有展开的分节才生成内容，避免重复的筛选控件 ID。
const researcherSections=new Set();
function researcherAgentsSection(){
  return filterBar('agents')+`<div id="directory-results">${directory('agents')}</div><div class="inspector-actions">${button('批量创建实例','spawn')}</div>`;
}
function researcherRunsSection(){
  return filterBar('runs')+`<div id="directory-results">${directory('runs')}</div>`;
}
function researcherEventsSection(){
  return events.length?table(['序号','事件','对象','时间','载荷'],events.slice(-150).reverse().map(e=>`<tr><td class="mono">#${e.seq}</td><td class="mono">${esc(e.kind)}</td><td class="mono">${esc(short(e.subject))}</td><td>${time(e.at)}</td><td><button class="text-button" data-action="event-detail" data-id="${e.seq}">检查 ↗</button></td></tr>`)):empty('暂无事件','开始操作后查看完整变化。');
}
function researcherFleetsSection(){
  const id=fleetId();
  if(id)return selectedFleet?.id===id?(selectedFleet.loadError?`<div class="callout" role="alert">${esc(selectedFleet.loadError)}</div>`:fleetPage(selectedFleet)):'<div class="loading">正在读取 Agent 池与操作…</div>';
  const fleets=data.fleets||[],capacity=data.capacity;
  return `<div class="quota-summary"><span>已占用或预留 <strong>${capacity.reserved} <span>/ ${capacity.limit}</span></strong></span><span>${capacity.active} 个实例未终止</span>${button('创建 Agent 池','spawn')}</div>`+
    (fleets.length?table(['Agent 池 / 母体','策略','目标 / 当前','就绪 / 排空','状态','操作'],fleets.map(f=>`<tr><td><div class="primary-text">${esc(f.preset_name)} <span class="mono">@ ${esc(f.preset_version)}</span></div><div class="secondary-text mono">${esc(short(f.id))}</div></td><td>${f.mode==='maintain'?'持续维持':'有限批次'}</td><td class="mono">${f.desired_count} / ${f.current_count}</td><td class="mono">${f.ready_count} / ${f.draining_count}</td><td>${badge(f.state)}</td><td><a class="text-button" href="#fleet/${esc(f.id)}">查看成员</a></td></tr>`)):empty('还没有 Agent 池','群体自行召集成员时也会创建 Agent 池。'));
}
// —— 群体机制参数：游走池、经验阈值、模式开关 ——
function researcherSwarmSection(){
  if(!colonyPool&&!colonyLoading)queueMicrotask(loadColony);
  const pool=colonyPool,experience=colonyExperience?.experience||[];
  const budget=Number(pool?.colony_budget)||0,spent=Number(pool?.colony_spent)||0;
  const poolTable=`<dl class="detail-kv"><dt>自选模式</dt><dd>${pool?.enabled?'开启':'关闭'}</dd><dt>待命池上限 / 实际</dt><dd class="mono">${esc(pool?.pool_max??'—')} / ${esc((pool?.peers||[]).length)}</dd><dt>选型模式</dt><dd class="mono">${esc(data.board?.router?.mode||'未提供')}</dd><dt>待命消耗</dt><dd class="mono">${esc(pool?.idle_burn_per_min??'—')} / 分钟</dd><dt>群体能量</dt><dd class="mono">${esc(Math.max(0,budget-spent))} / ${esc(budget)}</dd></dl>`;
  const expTable=experience.length?table(['母体 / 版本','成功 / 失败','能量花费','能量效率','响应阈值'],experience.map(e=>`<tr><td class="mono wrap">${esc(e.preset_name)} @ ${esc(e.preset_version)}</td><td class="mono">${esc(e.successes)} / ${esc(e.failures)}</td><td class="mono">${esc(e.energy_spent)}</td><td class="mono">${esc(e.cost_efficiency)}</td><td class="mono">${esc(e.response_threshold)}</td></tr>`)):'<p class="hint">尚无经验记录。</p>';
  const peers=pool?.peers||[];
  return poolTable+panel('母体经验与阈值',`<div class="panel-body">${expTable}</div>`)+(peers.length?`<details class="raw-data"><summary>游走池成员 JSON</summary>${dataView(peers)}</details>`:'');
}

// —— 任务机制详情：按需读取单个根任务的痕迹场/能量账本/裁决历史 ——
const researcherMechanisms=new Map();
function researcherMechanismSection(){
  const roots=(data.board?.tasks||[]).filter(t=>!t.parent_id).slice(0,30);
  if(!roots.length)return '<p class="hint">还没有根任务。</p>';
  return roots.map(t=>`<details class="researcher" data-mechanism="${esc(t.id)}"><summary>${esc(t.title)} <span class="mono hint">${esc(short(t.id))}</span></summary><div class="researcher-body" data-mechanism-body="${esc(t.id)}">展开后读取机制数据…</div></details>`).join('');
}
function researcherTraceTable(field){
  if(field?.loadError)return `<p class="hint">痕迹场读取失败：${esc(field.loadError)}</p>`;
  const traces=[...(field?.traces||[])].sort((a,b)=>boardTraceStrength(b)-boardTraceStrength(a));
  const rows=traces.length?table(['痕迹键','类型','浓度','沉积次数'],traces.map(trace=>`<tr><td class="mono">${esc(trace.key)}</td><td><span class="badge neutral">${esc(boardTraceKinds[trace.kind]||trace.kind)}</span></td><td><span class="trace-cell">${boardTraceBadge(trace)}<span class="mono">/ ${esc(field.max_strength??100)}</span></span></td><td class="mono">${esc(trace.deposits??0)}</td></tr>`)):'<p class="hint">该根任务还没有信息素痕迹。</p>';
  return rows+`<p class="hint">半衰期 ${boardTraceHalfLife(field?.half_life_ms)}，每隔该时长浓度减半。</p>`;
}
function researcherEnergyTable(ledger){
  if(!ledger)return '<p class="hint">暂无能量数据。</p>';
  const kinds={reserve:'预留',settle:'结算消耗',settle_unknown:'结果未知结算',accept_refund:'验收回充',complete_refund:'完成回充',refund:'回充',idle_burn:'待命消耗'};
  const rows=ledger.ledger||[];
  return `<dl class="detail-kv"><dt>预算 / 预留</dt><dd class="mono">${esc(ledger.budget)} / ${esc(ledger.reserved)}</dd><dt>已耗 / 回充 / 可用</dt><dd class="mono">${esc(ledger.spent)} / ${esc(ledger.refunded)} / ${esc(ledger.available)}</dd></dl>`+
    (rows.length?table(['运行','类型','数量','结余','时间'],rows.map(entry=>`<tr><td class="mono">${esc(short(entry.run_id||'—'))}</td><td>${esc(kinds[entry.kind]||entry.kind)}</td><td class="mono">${esc(entry.amount)}</td><td class="mono">${esc(entry.balance)}</td><td>${time(entry.created_at)}</td></tr>`)):'<p class="hint">尚无记账记录。</p>');
}
function researcherNotesTable(notes){
  if(!notes?.length)return '<p class="hint">尚无黑板记录。</p>';
  const status={active:'现行结论',superseded:'被裁决取代'};
  return table(['键','修订','状态','支持度','内容'],notes.map(n=>`<tr><td class="mono wrap">${esc(n.key)}</td><td class="mono">r${esc(n.revision)}</td><td>${esc(status[n.status]||n.status)}${n.superseded_by?`<div class="secondary-text mono">${esc(short(n.superseded_by))}</div>`:''}</td><td class="mono">${esc(n.support_score??n.support??'—')}</td><td class="wrap record-text">${esc(n.body)}</td></tr>`));
}
async function researcherLoadMechanism(taskId,body){
  if(researcherMechanisms.has(taskId)){body.innerHTML=researcherMechanisms.get(taskId);return;}
  body.innerHTML='<p class="hint">正在读取痕迹场、能量账本与裁决历史…</p>';
  const task=(data.board?.tasks||[]).find(t=>t.id===taskId);
  let traces,energy,notes;
  try{traces=await api(`/api/board/traces?root_id=${encodeURIComponent(task?.root_id||taskId)}`);}catch(error){traces={loadError:error.message};}
  try{energy=await api(`/api/board/tasks/${encodeURIComponent(taskId)}/energy`);}catch{energy=null;}
  try{notes=(await api(`/api/board/tasks/${encodeURIComponent(taskId)}/notes`)).notes;}catch{notes=null;}
  const html=panel('信息素痕迹场',`<div class="panel-body">${researcherTraceTable(traces)}</div>`)+
    panel('能量账本',`<div class="panel-body">${researcherEnergyTable(energy)}</div>`)+
    panel('黑板裁决历史',`<div class="panel-body">${researcherNotesTable(notes)}</div>`)+
    (task?.route_reason?`<details class="raw-data" open><summary>选型依据原文（route_reason）</summary><p class="record-text">${esc(task.route_reason)}</p></details>`:'');
  researcherMechanisms.set(taskId,html);
  body.innerHTML=html;
}

function researcherPage(){
  if(fleetId())researcherSections.add('fleets');
  const sections=[
    ['swarm','群体机制参数（池 / 经验阈值 / 模式）',researcherSwarmSection],
    ['mechanism','任务机制详情（痕迹场 / 能量账本 / 裁决历史）',researcherMechanismSection],
    ['runs','执行记录',researcherRunsSection],
    ['events','事件日志',researcherEventsSection],
    ['agents','Agent 实例目录',researcherAgentsSection],
    ['fleets','Agent 池',researcherFleetsSection],
    ['progress','实现进度',progressPage],
    ['messages','原始消息 Inbox / Outbox',messagesPage],
  ];
  return head('研究者视图：机制参数与原始记录')+
    '<p class="hint">这里是给研究协作机制的人准备的：原始事件、执行证据、池与消息记录。日常观察请用工作现场、蚁群与成果库。</p>'+
    sections.map(([id,title,body])=>`<details class="researcher" id="researcher-${id}" data-section="${id}" ${researcherSections.has(id)?'open':''}><summary>${title}</summary>${researcherSections.has(id)?`<div class="researcher-body">${body()}</div>`:''}</details>`).join('');
}
document.addEventListener('toggle',event=>{
  const el=event.target;
  if(!(el instanceof HTMLElement))return;
  // A re-render removes old nodes; their synthetic close events must not
  // clear the section set for the freshly rendered DOM.
  if(!el.isConnected)return;
  if(el.matches?.('details.researcher[data-mechanism]')){
    if(el.open&&page()==='researcher'){
      const body=el.querySelector('[data-mechanism-body]');
      if(body&&!body.dataset.loaded){body.dataset.loaded='1';researcherLoadMechanism(el.dataset.mechanism,body);}
    }
    return;
  }
  if(!el.matches?.('details.researcher[data-section]'))return;
  const id=el.dataset.section,had=researcherSections.has(id);
  if(el.open)researcherSections.add(id);else researcherSections.delete(id);
  if(had!==el.open&&page()==='researcher')render();
},true);