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
function researcherPage(){
  if(fleetId())researcherSections.add('fleets');
  const sections=[
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
  if(!(el instanceof HTMLElement)||!el.matches?.('details.researcher[data-section]'))return;
  const id=el.dataset.section,had=researcherSections.has(id);
  if(el.open)researcherSections.add(id);else researcherSections.delete(id);
  if(had!==el.open&&page()==='researcher')render();
},true);