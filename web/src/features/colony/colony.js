'use strict';

// 群体行为的一句话翻译：colony 页与 studio 现场共用。
// 输入一条持久事件，输出人类观察者能读懂的一句话；无法翻译时返回 null（不显示）。
const swarmKinds=new Set(['swarm.self_selected','swarm.peer_spawned','swarm.peer_retired','swarm.colony_starved','board.peer_recruited','board.task_claimed','board.task_split','board.children_activated','board.continuation_ready','board.trace_deposited','board.audit_settled','board.task_reviewed','board.task_requeued','board.rework_cooldown','board.note_adjudicated','board.energy_posted']);
function swarmTaskTitle(id,snapshot){
  const task=(snapshot?.board?.tasks||[]).find(t=>t.id===id);
  return task?.title||`任务 ${String(id??'').slice(0,8)}`;
}
function swarmMemberName(id,snapshot){
  const instance=(snapshot?.instances||[]).find(a=>a.id===id);
  return instance?`${instance.preset_name} #${String(instance.id).slice(-4)}`:`成员 #${String(id??'').slice(-4)}`;
}
function swarmNarrative(event,snapshot){
  if(!event||!swarmKinds.has(event.kind))return null;
  const d=event.data||{},subject=event.subject,title=()=>swarmTaskTitle(subject,snapshot);
  switch(event.kind){
    case 'swarm.self_selected':return `一只${d.preset_name||'成员'}主动接了「${title()}」`;
    case 'board.task_claimed':return `${swarmMemberName(d.instance_id||subject,snapshot)}开始处理「${title()}」`;
    case 'board.task_split':return `「${swarmTaskTitle(d.parent_id||subject,snapshot)}」被拆成${(d.task_ids||[]).length||d.count||'多'}份独立工作`;
    case 'board.children_activated':return '子任务开工';
    case 'board.continuation_ready':return '子任务全部完成，等待汇总';
    case 'board.peer_recruited':return '有同伴被带来协作';
    case 'board.trace_deposited':
      if(d.reason==='reuse')return '一份成果被同伴参考';
      if(d.reason==='accepted')return '质检通过，成果被认可';
      if(d.reason==='failure')return '一条路径失败，群体将换方向';
      if(d.reason==='recruitment')return '成功路径被加强，吸引更多同伴';
      return null;
    case 'board.audit_settled':
      if(d.state==='passed')return '独立质检通过';
      if(d.state==='changes-requested')return '质检退回，附修改意见';
      if(d.state==='inconclusive')return '质检证据不足，交人类定夺';
      return null;
    case 'board.task_reviewed':
      if(d.author_kind==='verifier'){
        if(d.decision==='accepted')return '独立质检通过';
        if(d.decision==='changes_requested')return '质检退回，附修改意见';
        return null;
      }
      if(d.author_kind==='operator')return `人类评审：${d.decision==='accepted'?'通过':'退回'}`;
      return null;
    case 'board.task_requeued':return '一次尝试失败，工作重新开放';
    case 'board.rework_cooldown':return '反复修改仍未达标，暂缓重试';
    case 'swarm.peer_spawned':return '群体补充了新成员';
    case 'swarm.peer_retired':return '一名成员退出';
    case 'swarm.colony_starved':return '群体预算耗尽，成员休息';
    case 'board.note_adjudicated':return '两个结论冲突，证据更充分的一方胜出';
    case 'board.energy_posted':return d.kind==='idle_burn'?null:'一笔花费被记录';
    default:return null;
  }
}
// 最近 count 条可翻译的群体动态（倒序），供现场与蚁群页共用。
function swarmFeed(list,count,snapshot){
  const items=[];
  for(let i=list.length-1;i>=0&&items.length<count;i--){
    const text=swarmNarrative(list[i],snapshot);
    if(text)items.push({text,at:list[i].at});
  }
  return items;
}
if(typeof window!=='undefined'){window.swarmNarrative=swarmNarrative;window.swarmFeed=swarmFeed;}

// —— 蚁群页 ——
let colonyPool=null,colonyExperience=null,colonyLoading=false,colonyLoadError='';
async function loadColony(){
  if(colonyLoading)return;colonyLoading=true;colonyLoadError='';
  try{[colonyPool,colonyExperience]=await Promise.all([api('/api/swarm/pool'),api('/api/presets/experience')]);}
  catch(error){colonyLoadError=error.message;}
  finally{colonyLoading=false;if(page()==='colony'||page()==='researcher')render();}
}
const colonyMemberStates={busy:'正在干活',ready:'待命',draining:'收尾中',suspended:'休眠中'};
function colonyMemberRows(snapshot=data){
  const peers=new Map((colonyPool?.peers||[]).map(p=>[p.instance_id,p]));
  const experience=new Map((colonyExperience?.experience||[]).map(e=>[`${e.preset_name}@${e.preset_version}`,e]));
  const tasks=snapshot.board?.tasks||[],agents=snapshot.board?.agents||[];
  const members=snapshot.instances.filter(a=>a.state!=='terminated');
  for(const p of peers.values())if(!members.some(a=>a.id===p.instance_id))members.push({id:p.instance_id,preset_name:p.preset_name,preset_version:p.preset_version,state:p.state,poolOnly:true});
  return members.map(a=>{
    const preset=(snapshot.presets||[]).find(p=>p.name===a.preset_name);
    const exp=experience.get(`${a.preset_name}@${a.preset_version}`);
    const total=exp?(exp.successes||0)+(exp.failures||0):0;
    const claimed=agents.find(g=>g.instance_id===a.id&&g.claimed_task_id);
    const task=claimed?tasks.find(t=>t.id===claimed.claimed_task_id):tasks.find(t=>t.instance_id===a.id&&t.state==='running');
    return {id:a.id,nickname:`${a.preset_name} #${String(a.id).slice(-4)}`,purpose:preset?.definition?.purpose||'未记录擅长方向',
      done:exp?`${exp.successes||0} 单成功${exp.failures?` / ${exp.failures} 单未成`:''}`:'还没有记录',
      rate:total?`${Math.round((exp.successes||0)/total*100)}%`:'—',
      state:colonyMemberStates[a.state]||a.state,rawState:a.state,task:task?.title||'',preset_version:a.preset_version};
  });
}
function colonyDuration(ms){
  if(!Number.isFinite(ms)||ms<0)return '—';
  const minutes=Math.round(ms/60000);
  if(minutes<60)return `${minutes} 分钟`;
  return `${(minutes/60).toFixed(1)} 小时`;
}
function colonyCostRows(snapshot=data){
  const tasks=snapshot.board?.tasks||[];
  return tasks.filter(t=>!t.parent_id).map(t=>{
    const accepted=(t.reviews||[]).filter(r=>r.decision==='accepted').sort((a,b)=>(b.created_at||0)-(a.created_at||0))[0];
    const finished=accepted?.created_at??t.finished_at;
    return {id:t.id,title:t.title,spent:t.energy?.spent??0,budget:t.energy?.budget??0,
      delivered:Boolean(accepted),duration:finished&&t.created_at?colonyDuration(finished-t.created_at):'进行中'};
  });
}
function colonyPage(){
  if(!colonyPool&&!colonyLoading&&!colonyLoadError)queueMicrotask(loadColony);
  const members=colonyMemberRows(),feed=swarmFeed(events,30,data);
  const budget=Number(colonyPool?.colony_budget)||0,spent=Number(colonyPool?.colony_spent)||0;
  const remaining=Math.max(0,budget-spent),percent=budget?Math.min(100,spent/budget*100):0;
  const costs=colonyCostRows();
  return head('蚁群', button(colonyLoading?'正在刷新…':'刷新','colony-refresh',false,colonyLoading?'disabled':''))+
    (colonyLoadError?`<div class="callout" role="alert">${esc(colonyLoadError)}。${button('重试','colony-refresh')}</div>`:'')+
    panel('成员花名册',members.length?table(['成员','擅长','干了几单','成功率','当前状态','正在做'],members.map(m=>`<tr><td><div class="primary-text">${esc(m.nickname)}</div><div class="secondary-text mono">${esc(short(m.id))}</div></td><td class="wrap">${esc(m.purpose)}</td><td>${esc(m.done)}</td><td>${esc(m.rate)}</td><td>${badge(m.rawState==='busy'?'running':m.rawState==='ready'?'ready':m.rawState==='draining'?'draining':'terminated',m.state)}</td><td class="wrap">${m.task?esc(m.task):'<span class="hint">—</span>'}</td></tr>`)):empty('还没有成员','发布任务后，群体会自行召集合适的成员。'),`<span class="board-count">${members.length} 名在场</span>`)+
    panel('群体动态',feed.length?`<div class="panel-body"><div class="swarm-timeline">${feed.map(item=>`<div class="swarm-line"><span class="swarm-line-text">${esc(item.text)}</span><span class="swarm-line-time">${time(item.at)}</span></div>`).join('')}</div></div>`:empty('群体还很安静','成员开始协作后，这里会出现它们的一举一动。'))+
    panel('开销',`<div class="panel-body">${budget?`<div class="energy-progress"><span>群体总预算还剩 ${remaining.toLocaleString()} / ${budget.toLocaleString()}</span><div class="progress-track ${percent>=66?'is-low':percent>=33?'is-mid':''}" role="img" aria-label="群体预算已用 ${Math.round(percent)}%"><i style="width:${percent.toFixed(1)}%"></i></div></div>`:'<p class="hint">群体池尚未启用，暂无总预算。</p>'}
      ${costs.length?table(['工作','已花费','从发布到交付'],costs.map(c=>`<tr><td class="wrap"><a href="#board/${esc(c.id)}">${esc(c.title)}</a></td><td class="mono">${Number(c.spent).toLocaleString()}${c.budget?` <span class="hint">/ 预算 ${Number(c.budget).toLocaleString()}</span>`:''}</td><td>${c.delivered?esc(c.duration):`<span class="hint">${esc(c.duration)}</span>`}</td></tr>`)):'<p class="hint">还没有已发布的工作。</p>'}</div>`);
}
async function colonyAction(action){
  if(action==='colony-refresh'){await loadColony();return true;}
  return false;
}