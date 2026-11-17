'use strict';

// 研究者视图：机制参数与原始记录的聚合入口。全部内容直接呈现，不使用折叠。
function researcherAgentsSection(){
  return `<div>${agentsTable(data.instances)}</div><div class="inspector-actions">${button('批量创建实例','spawn')}</div>`;
}
function researcherRunsSection(){
  return `<div>${runsTable(data.runs)}</div>`;
}
function researcherEventsSection(){
  return events.length?table(['序号','事件','对象','时间','载荷'],events.slice(-150).reverse().map(e=>`<tr><td class="mono">#${e.seq}</td><td class="mono">${esc(e.kind)}</td><td class="mono">${esc(short(e.subject))}</td><td>${time(e.at)}</td><td><button class="text-button" data-action="event-detail" data-id="${e.seq}">检查 ↗</button></td></tr>`)):empty('暂无事件','开始操作后查看完整变化。');
}
function researcherMessagesSection(){
  const list=data.messages||[];
  return list.length?table(['消息','状态','发送方','接收方','时间'],list.slice(0,30).map(m=>`<tr><td class="mono">${esc(short(m.id))}</td><td>${esc(m.delivery_state||'—')}</td><td class="mono wrap">${esc(short(m.sender_instance_id||'操作者'))}</td><td class="mono wrap">${esc(short(m.recipient_id||'—'))}</td><td>${time(m.created_at)}</td></tr>`)):empty('暂无消息','成员之间发生交接后，消息记录会出现在这里。');
}
function researcherFleetsSection(){
  const id=fleetId();
  if(id)return selectedFleet?.id===id?(selectedFleet.loadError?`<div class="callout" role="alert">${esc(selectedFleet.loadError)}</div>`:fleetPage(selectedFleet)):'<div class="loading">正在读取 Agent 池与操作…</div>';
  const fleets=data.fleets||[],capacity=data.capacity;
  return `<div class="quota-summary"><span>已占用或预留 <strong>${capacity.reserved} <span>/ ${capacity.limit}</span></strong></span><span>${capacity.active} 个实例未终止</span>${button('创建 Agent 池','spawn')}</div>`+
    (fleets.length?table(['Agent 池 / Preset','策略','目标 / 当前','就绪 / 排空','状态','操作'],fleets.map(f=>`<tr><td><div class="primary-text">${esc(f.preset_name)} <span class="mono">@ ${esc(f.preset_version)}</span></div><div class="secondary-text mono">${esc(short(f.id))}</div></td><td>${f.mode==='maintain'?'持续维持':'有限批次'}</td><td class="mono">${f.desired_count} / ${f.current_count}</td><td class="mono">${f.ready_count} / ${f.draining_count}</td><td>${badge(f.state)}</td><td><a class="text-button" href="#fleet/${esc(f.id)}">查看成员</a></td></tr>`)):empty('还没有 Agent 池','群体自行召集成员时也会创建 Agent 池。'));
}
// —— 群体机制参数：游走池、经验阈值、模式开关 ——
function researcherSwarmSection(){
  if(!colonyPool&&!colonyLoading)queueMicrotask(loadColony);
  const pool=colonyPool,experience=colonyExperience?.experience||[];
  const budget=Number(pool?.colony_budget)||0,spent=Number(pool?.colony_spent)||0;
  const poolTable=`<dl class="detail-kv"><dt>自选模式</dt><dd>${pool?.enabled?'开启':'关闭'}</dd><dt>待命池上限 / 实际</dt><dd class="mono">${esc(pool?.pool_max??'—')} / ${esc((pool?.peers||[]).length)}</dd><dt>选型模式</dt><dd class="mono">${esc(data.board?.router?.mode||'未提供')}</dd><dt>待命消耗</dt><dd class="mono">${esc(pool?.idle_burn_per_min??'—')} / 分钟</dd><dt>群体能量</dt><dd class="mono">${esc(Math.max(0,budget-spent))} / ${esc(budget)}</dd></dl>`;
  const expTable=experience.length?table(['Preset / 版本','成功 / 失败','能量花费','能量效率','响应阈值'],experience.map(e=>`<tr><td class="mono wrap">${esc(e.preset_name)} @ ${esc(e.preset_version)}</td><td class="mono">${esc(e.successes)} / ${esc(e.failures)}</td><td class="mono">${esc(e.energy_spent)}</td><td class="mono">${esc(e.cost_efficiency)}</td><td class="mono">${esc(e.response_threshold)}</td></tr>`)):'<p class="hint">尚无经验记录。</p>';
  return poolTable+panel('Preset 经验与阈值',`<div class="panel-body">${expTable}</div>`);
}

// —— 任务机制概览：直接取自快照，逐根任务呈现 ——
function researcherMechanismSection(){
  const roots=(data.board?.tasks||[]).filter(t=>!t.parent_id).slice(0,30);
  if(!roots.length)return '<p class="hint">还没有根任务。</p>';
  return table(['任务','痕迹','能量','黑板结论','操作'],roots.map(t=>{
    const trace=t.trace?`${Number(t.trace.strength).toFixed(1)}（沉积 ${Number(t.trace.deposits||0)}）`:'—';
    const energy=t.energy?`${Number(t.energy.available).toLocaleString()} / ${Number(t.energy.budget).toLocaleString()}`:'—';
    const notes=(t.notes||[]).filter(n=>n.status==='active').length;
    return `<tr><td><div class="primary-text wrap">${esc(t.title)}</div><div class="secondary-text mono">${esc(short(t.id))}</div></td><td class="mono">${esc(trace)}</td><td class="mono">${esc(energy)}</td><td>${notes?`${notes} 条`:'<span class="hint">—</span>'}</td><td><a class="text-button" href="#board/${esc(t.id)}">任务详情 ↗</a></td></tr>`;
  }));
}

// —— 成员私有文件浏览 ——
function researcherPrivateFilesSection(){
  if(!workspaceCatalog&&!workspaceLoading)queueMicrotask(loadWorkspaces);
  return deliveriesPrivateFiles();
}

// —— 分页呈现：一次只看一类记录，避免整页滚动 ——
let researcherTab='swarm';
const researcherTabs=[
  ['swarm','群体机制参数'],
  ['mechanism','任务机制概览'],
  ['runs','执行记录'],
  ['events','事件日志'],
  ['agents','实例与 Agent 池'],
  ['progress','实现进度'],
  ['records','消息与私有文件'],
];
function researcherTabBar(){
  return `<div class="tabs" role="tablist">${researcherTabs.map(([key,label])=>`<button role="tab" data-action="researcher-tab" data-id="${key}" aria-selected="${key===researcherTab}" aria-pressed="${key===researcherTab}" class="${key===researcherTab?'active':''}">${label}</button>`).join('')}</div>`;
}
function researcherTabBody(){
  if(researcherTab==='mechanism')return researcherMechanismSection();
  if(researcherTab==='runs')return researcherRunsSection();
  if(researcherTab==='events')return researcherEventsSection();
  if(researcherTab==='agents')return researcherAgentsSection()+researcherFleetsSection();
  if(researcherTab==='progress')return progressBody();
  if(researcherTab==='records')return panel('最近消息',`<div class="panel-body">${researcherMessagesSection()}</div>`)+panel('成员私有文件',`<div class="panel-body">${researcherPrivateFilesSection()}</div>`);
  return researcherSwarmSection();
}
function researcherPage(){
  if(fleetId())researcherTab='agents';
  const active=researcherTabs.find(([key])=>key===researcherTab)||researcherTabs[0];
  return head('研究者视图：机制参数与原始记录')+
    '<p class="hint">这里是给研究协作机制的人准备的：原始事件、执行证据、池与消息记录。日常观察请用工作现场、蚁群与成果库。</p>'+
    researcherTabBar()+
    `<div class="researcher-tab" role="tabpanel">${panel(active[1],`<div class="panel-body">${researcherTabBody()}</div>`)}</div>`;
}
