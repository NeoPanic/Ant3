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
    (fleets.length?table(['Agent 池 / Preset','策略','目标 / 当前','就绪 / 排空','状态','操作'],fleets.map(f=>`<tr><td><div class="primary-text">${esc(f.preset_name)} <span class="mono">@ ${esc(f.preset_version)}</span></div><div class="secondary-text mono">${esc(short(f.id))}</div></td><td>${f.mode==='maintain'?'持续维持':'有限批次'}</td><td class="mono">${f.desired_count} / ${f.current_count}</td><td class="mono">${f.ready_count} / ${f.draining_count}</td><td>${badge(f.state)}</td><td><a class="text-button" href="/fleet/${esc(f.id)}">查看成员</a></td></tr>`)):empty('还没有 Agent 池','群体自行召集成员时也会创建 Agent 池。'));
}
// —— 群体机制参数：可编辑并持久化，保存后下一次调度生效 ——
function researcherSwarmSection(){
  if(!colonyPool&&!colonyLoading)queueMicrotask(loadColony);
  const pool=colonyPool,experience=colonyExperience?.experience||[];
  const budget=Number(pool?.colony_budget)||0,spent=Number(pool?.colony_spent)||0;
  const t=data.settings?.traces||{},s=data.settings?.self_select||{},e=data.settings?.energy||{};
  const num=(value,fallback)=>value??fallback;
  const settingsForm=`<form data-form="settings" class="mechanism-settings">
    <fieldset><legend>自选模式</legend>
      <label class="capability-choice"><input type="checkbox" name="self_select_enabled" ${s.enabled?'checked':''}><span><strong>允许空闲成员自己挑活</strong><small>关闭时由控制面统一安排；开启后空闲实例进入游走池，按响应阈值自行抢单。</small></span></label>
      <div class="grid-three">
        <label>待命池上限<input name="pool_max" type="number" min="1" max="64" step="1" value="${esc(num(s.pool_max,8))}" required></label>
        <label>每分钟待命消耗<input name="idle_burn_per_min" type="number" min="0" max="100000" step="1" value="${esc(num(s.idle_burn_per_min,20))}" required></label>
        <label>群体能量池<input name="colony_budget" type="number" min="0" max="10000000" step="1000" value="${esc(num(s.colony_budget,200000))}" required></label>
        <label>连续失败淘汰阈值<input name="poor_failures" type="number" min="1" max="100" step="1" value="${esc(num(s.poor_failures,3))}" required></label>
      </div>
    </fieldset>
    <fieldset><legend>信息素痕迹</legend>
      <div class="grid-three">
        <label>半衰期（分钟）<input name="half_life_minutes" type="number" min="1" max="1440" step="1" value="${esc(num(t.half_life_minutes,30))}" required></label>
        <label>最大浓度<input name="max_strength" type="number" min="1" max="10000" step="1" value="${esc(num(t.max_strength,100))}" required></label>
        <label>局部感知阈值<input name="perception_threshold" type="number" min="0" step="0.1" value="${esc(num(t.perception_threshold,1))}" required></label>
      </div>
      <label class="capability-choice"><input type="checkbox" name="exploration" ${t.exploration?'checked':''}><span><strong>任务排序加入概率探索</strong><small>大部分时候按痕迹强度排序，偶尔随机换方向，避免路径锁死。</small></span></label>
    </fieldset>
    <fieldset><legend>能量结算</legend>
      <div class="grid-three">
        <label>默认任务预算<input name="default_budget" type="number" min="2000" max="10000000" step="1000" value="${esc(num(e.default_budget,100000))}" required></label>
        <label>每次运行预留<input name="reserve" type="number" min="0" max="1000000" step="100" value="${esc(num(e.reserve,2000))}" required></label>
        <label>结果未知成本<input name="unknown_cost" type="number" min="0" max="1000000" step="100" value="${esc(num(e.unknown_cost,1500))}" required></label>
        <label>完成回充<input name="complete_refund" type="number" min="0" max="1000000" step="100" value="${esc(num(e.complete_refund,500))}" required></label>
        <label>验收回充<input name="accept_refund" type="number" min="0" max="1000000" step="100" value="${esc(num(e.accept_refund,4000))}" required></label>
        <label>每能量 Token 数<input name="tokens_per_energy" type="number" min="1" max="10000" step="1" value="${esc(num(e.tokens_per_energy,10))}" required></label>
      </div>
    </fieldset>
    <div id="form-error" class="error" role="alert"></div>
    <p class="hint">保存后从下一次扫描、沉积或结算开始生效；选型模式与模型协议由启动配置决定。</p>
    <div class="form-actions"><button class="button primary" type="submit">保存机制参数</button></div>
  </form>`;
  const poolTable=`<dl class="detail-kv"><dt>选型模式</dt><dd class="mono">${esc(data.board?.router?.mode||'未提供')}</dd><dt>群体能量</dt><dd class="mono">${esc(Math.max(0,budget-spent))} / ${esc(budget)}</dd></dl>`;
  const expTable=experience.length?table(['Preset / 版本','成功 / 失败','能量花费','能量效率','响应阈值'],experience.map(e=>`<tr><td class="mono wrap">${esc(e.preset_name)} @ ${esc(e.preset_version)}</td><td class="mono">${esc(e.successes)} / ${esc(e.failures)}</td><td class="mono">${esc(e.energy_spent)}</td><td class="mono">${esc(e.cost_efficiency)}</td><td class="mono">${esc(e.response_threshold)}</td></tr>`)):'<p class="hint">尚无经验记录。</p>';
  const peers=pool?.peers||[];
  const peerTable=peers.length?panel('游走池成员',`<div class="panel-body">${table(['成员','Preset','状态'],peers.map(p=>`<tr><td class="mono">${esc(short(p.instance_id))}</td><td>${esc(p.preset_name)} <span class="mono">@ ${esc(p.preset_version)}</span></td><td>${badge(p.state)}</td></tr>`))}</div>`):'';
  return settingsForm+poolTable+panel('Preset 经验与阈值',`<div class="panel-body">${expTable}</div>`)+peerTable;
}

// —— 任务机制概览：直接取自快照，逐根任务呈现 ——
function researcherMechanismSection(){
  const roots=(data.board?.tasks||[]).filter(t=>!t.parent_id).slice(0,30);
  if(!roots.length)return '<p class="hint">还没有根任务。</p>';
  return table(['任务','痕迹','能量','黑板结论','操作'],roots.map(t=>{
    const trace=t.trace?`${Number(t.trace.strength).toFixed(1)}（沉积 ${Number(t.trace.deposits||0)}）`:'—';
    const energy=t.energy?`${Number(t.energy.available).toLocaleString()} / ${Number(t.energy.budget).toLocaleString()}`:'—';
    const notes=(t.notes||[]).filter(n=>n.status==='active').length;
    return `<tr><td><div class="primary-text wrap">${esc(t.title)}</div><div class="secondary-text mono">${esc(short(t.id))}</div></td><td class="mono">${esc(trace)}</td><td class="mono">${esc(energy)}</td><td>${notes?`${notes} 条`:'<span class="hint">—</span>'}</td><td><a class="text-button" href="/board/${esc(t.id)}">任务详情 ↗</a></td></tr>`;
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
  return head('研究者视图：机制参数与原始记录')+
    '<p class="hint">这里是给研究协作机制的人准备的：原始事件、执行证据、池与消息记录。日常观察请用工作现场、蚁群与成果库。</p>'+
    tabBar('researcher-tab',researcherTabs,researcherTab)+
    `<div class="researcher-tab" role="tabpanel">${tabPanel(researcherTabBody())}</div>`;
}
