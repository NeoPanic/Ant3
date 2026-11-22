'use strict';
const chat={events:[],source:null,request:0,connecting:false,loaded:false,error:'',status:'正在连接',draft:'',pending:null,sending:false,before:null,hasMore:false,follow:true};
function chatTaskId(){return location.pathname.startsWith('/chat/')?routeParts()[1]||'': '';}
function chatRoots(){return (data?.board?.tasks||[]).filter(t=>!t.parent_id).sort((a,b)=>b.created_at-a.created_at||b.id.localeCompare(a.id));}
function chatTask(id){return (data?.board?.tasks||[]).find(t=>t.id===id);}
function chatMember(event){
  const instance=event.instance_id||(data?.board?.agents||[]).find(a=>a.run_id===(event.run_id||event.data?.run_id))?.instance_id;
  const preset=event.preset_name||(data?.instances||[]).find(i=>i.id===instance)?.preset_name;
  return instance?`${preset||'蚂蚁'} ${String(instance).slice(-6)}`:'蚁群';
}
function chatEventText(event){
  const d=event.data||{},task=chatTask(event.task_id),title=task?.parent_id?`「${task.title}」`:'这项工作';
  switch(event.kind){
    case 'board.task_claimed':return `接下了${title}。`;
    case 'board.peer_recruited':return '通过同伴交接加入了这项工作。';
    case 'board.task_split':return `将工作拆成 ${d.task_ids?.length||0} 项，等待同伴接手。`;
    case 'board.progress_updated':return d.message||'';
    case 'execution.model_started':return '正在处理任务，等待模型返回。';
    case 'execution.model_completed':return d.text||'';
    case 'execution.text_live':return d.text||'';
    case 'execution.tool_started':return `正在${toolLabel(d.name||'')}。`;
    case 'execution.tool_completed':return d.error?`${toolLabel(d.name||'')}未完成：${typeof d.error==='string'?d.error:d.error.code||'工具返回错误'}。`:'';
    case 'execution.finished':return d.error?`${d.error.message||runErrorText(d.error.code||d.error)}。`:'';
    case 'board.task_reviewed':return d.decision==='accepted'?'人工验收通过。':`退回修改：${d.feedback||'按评审意见继续工作'}`;
    case 'board.task_settled':return d.state==='cancelled'?`${title}已取消，已有记录保留。`:d.state==='failed'?`${title}未完成${d.error==='energy_exhausted'?'，预算已耗尽':''}。`:d.state==='interrupted'?`${title}执行结果未知，需要核对。`:'';
    case 'board.routing_deferred':return `${boardRoutingMessage(d.error)}，稍后自动重试。`;
    default:return '';
  }
}
function chatRows(){
  const selected=chatTaskId(),roots=chatRoots().filter(t=>!selected||t.id===selected);
  const rows=roots.map(task=>({key:`user-${task.id}`,at:task.created_at,seq:-1,task,user:true}));
  const ids=new Set(roots.map(t=>t.id));
  const live=new Map(),ended=new Set();
  for(const event of chat.events){
    if(!ids.has(event.root_id)||!['execution.text_delta','execution.text_end'].includes(event.kind))continue;
    const key=`${event.run_id}:${event.data.generation}:${event.root_id}`;
    let message=live.get(key);
    if(!message){message={...event,kind:'execution.text_live',data:{text:'',offset:0,partial:false}};live.set(key,message);}
    if(event.kind==='execution.text_end'){
      message.data.text=event.data.text||'';message.data.error=event.data.error;message.data.partial=false;
      ended.add(`${event.run_id}:${event.data.text}`);
    }else{
      if(event.data.offset===0)message.data.text='';
      else if(message.data.offset!==event.data.offset)message.data.partial=true;
      message.data.text+=event.data.text;message.data.offset=event.data.offset+new TextEncoder().encode(event.data.text).length;
    }
  }
  for(const event of [...chat.events,...live.values()]){
    if(!ids.has(event.root_id)||!chatEventText(event))continue;
    if(['execution.text_delta','execution.text_end'].includes(event.kind))continue;
    if(event.kind==='execution.model_completed'&&ended.has(`${event.run_id}:${event.data.text}`))continue;
    if(event.kind==='execution.model_started'&&chat.events.some(e=>e.run_id===event.run_id&&e.seq>event.seq&&['execution.text_delta','execution.model_completed','execution.finished'].includes(e.kind)))continue;
    if(event.kind==='execution.tool_started'&&chat.events.some(e=>e.run_id===event.run_id&&e.seq>event.seq&&e.kind==='execution.tool_completed'&&e.data.name===event.data.name))continue;
    rows.push({key:`event-${event.seq}-${event.root_id}`,at:event.at,seq:event.seq,event});
  }
  return rows.sort((a,b)=>a.at-b.at||a.seq-b.seq);
}
function chatAvatar(user=false){
  return `<span class="chat-avatar" aria-hidden="true">${user?'<svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><circle cx="12" cy="8" r="3.5"/><path d="M5 20v-1a7 7 0 0 1 14 0v1"/></svg>':'<img src="/mark.svg" width="26" height="26" alt="">'}</span>`;
}
function chatRow(row){
  if(row.user){const t=row.task;return `<article class="chat-message is-user" id="${esc(row.key)}">${chatAvatar(true)}<div class="chat-message-content"><div class="chat-message-meta"><strong>你</strong><time>${esc(time(row.at))}</time><a href="/chat/${esc(t.id)}">${badge(boardBadgeClass(t),boardStatus(t,boardRoute(t.id)))}</a></div><div class="chat-bubble">${esc(t.instructions)}</div></div></article>`;}
  const e=row.event,text=chatEventText(e),spoken=['execution.model_completed','execution.text_live','board.progress_updated'].includes(e.kind);
  const failure=(e.kind==='execution.finished'&&e.data?.error)||e.kind==='execution.tool_completed'||['failed','interrupted'].includes(e.data?.state);
  const root=chatTask(e.root_id);
  return `<article class="chat-message ${spoken?'is-member':'is-activity'} ${failure?'is-error':''}" id="${esc(row.key)}">${chatAvatar()}<div class="chat-message-content"><div class="chat-message-meta"><strong>${esc(chatMember(e))}</strong><time>${esc(time(e.at))}</time>${!chatTaskId()&&root?`<a href="/chat/${esc(e.root_id)}" title="只看这项任务">${esc(root.title)}</a>`:''}</div>${e.data.partial?'<p class="hint">部分文字未载入，可加载更早的动态。</p>':''}<div class="${spoken?'chat-bubble':'chat-activity-text'}">${esc(text)}</div>${e.kind==='execution.text_live'&&e.data.error?'<p class="chat-problem">本次生成中断，以上为已收到的文字。</p>':''}</div></article>`;
}
function chatSummary(task){
  const tree=(data?.board?.tasks||[]).filter(t=>t.root_id===task.id),route=boardRoute(task.id);
  const problems=tree.filter(t=>['failed','interrupted','blocked'].includes(t.state));
  const memberIds=new Set((data?.board?.agents||[]).filter(a=>a.root_id===task.id).map(a=>a.instance_id));
  for(const e of chat.events)if(e.root_id===task.id&&e.instance_id)memberIds.add(e.instance_id);
  const result=task.result?.output;
  const seen=chat.events.some(e=>e.root_id===task.id&&['execution.model_completed','execution.text_end'].includes(e.kind)&&e.data?.text===result);
  return `<section class="chat-task-summary" aria-label="当前任务状态"><div class="chat-summary-line">${badge(boardBadgeClass(task),boardStatus(task,route))}<span>${memberIds.size} 位成员参与${tree.length>1?`，${tree.filter(t=>t.state==='completed').length} / ${tree.length} 项执行完成`:''}</span><a href="/board/${esc(task.id)}" class="text-button">任务详情</a></div>${route?.error?`<p class="chat-problem" role="status">${esc(boardRoutingMessage(route.error))}${route.retry_at?`，下次尝试 ${esc(time(route.retry_at))}`:''}。</p>`:''}${problems.length?`<p class="chat-problem">${problems.length} 项执行需要检查：${problems.map(t=>`<a href="/board/${esc(t.id)}">${esc(t.title)}</a>`).join('、')}</p>`:''}${task.progress&&!chat.events.some(e=>e.task_id===task.id&&e.data?.message===task.progress.message)?`<p>${esc(task.progress.message)}</p>`:''}${result&&!seen?`<details id="chat-result-${esc(task.id)}"><summary>本轮成果</summary><div class="chat-bubble">${esc(result)}</div>${task.result.output_truncated?'<p class="hint">这里只显示成果预览，完整文本见执行记录。</p>':''}</details>`:''}<div class="chat-summary-actions">${task.run_id?button('执行记录','run-detail',false,`data-id="${esc(task.run_id)}"`):''}${task.review_state==='pending'&&!boardAuditing(task)?button('评审成果','board-review',false,`data-id="${esc(task.id)}" data-run="${esc(task.run_id)}"`):''}${!boardTerminal.has(task.state)?button('取消任务','board-cancel',false,`data-id="${esc(task.id)}"`):''}</div></section>`;
}
function chatRender(){
  if(!$('#chat-surface')){
    $('#content').innerHTML=`<div id="chat-surface"><div class="chat-heading"><h1>蚁群群聊</h1>${button('查看全部','chat-all')}${button('刷新','chat-refresh')}</div><div class="chat-layout"><aside class="chat-task-list" aria-label="对话中的任务"><h2>任务</h2><div id="chat-tasks"></div></aside><section class="chat-conversation" aria-label="蚁群对话"><div id="chat-current"></div><p id="chat-stream-status" role="status"></p><div id="chat-feed" tabindex="0" aria-label="对话记录"></div><div class="chat-composer"><form id="chat-form"><label for="chat-input">交给蚁群的任务</label><textarea id="chat-input" name="text" rows="3" maxlength="8000" placeholder="说说你想完成什么，蚁群会自行接单。" required></textarea><div id="chat-send-error" class="error" role="alert" tabindex="-1"></div><div class="chat-composer-bottom"><span class="hint">每次发送是一项任务，执行会调用模型并消耗能量。⌘ / Ctrl + Enter 发送</span><button class="button primary" type="submit">发送任务</button></div></form></div></section></div></div>`;
    $('#chat-input').value=chat.draft;
    $('#chat-input').addEventListener('input',e=>{chat.draft=e.target.value;});
    $('#chat-input').addEventListener('keydown',e=>{if(e.key==='Enter'&&(e.metaKey||e.ctrlKey)&&!e.isComposing&&e.keyCode!==229){e.preventDefault();$('#chat-form').requestSubmit();}});
    $('#chat-feed').addEventListener('scroll',()=>{const feed=$('#chat-feed');chat.follow=feed.scrollHeight-feed.scrollTop-feed.clientHeight<70;});
    $('#chat-form').addEventListener('submit',chatSend);
  }
  const selected=chatTaskId(),roots=chatRoots(),task=chatTask(selected);
  const list=roots.map(t=>`<a class="chat-task-link ${t.id===selected?'is-selected':''}" href="/chat/${esc(t.id)}" ${t.id===selected?'aria-current="page"':''}><strong>${esc(t.title)}</strong><span>${esc(boardStatus(t,boardRoute(t.id)))}</span></a>`).join('');
  chatReplace('#chat-tasks',list||'<p class="hint">发送后，任务会出现在这里。</p>');
  chatReplace('#chat-current',task?chatSummary(task):selected?'<p class="chat-problem">这项任务不在当前快照中，请在任务板检查。</p>':'');
  const status=chat.error||`${chat.status}${chat.loaded?'，工作动态会持续出现在对话中':''}`;
  $('#chat-stream-status').textContent=status;
  const rows=chatRows(),history=chat.hasMore?`${button('加载更早的动态','chat-history')}<p class="hint">当前只展示部分执行动态，任务状态以最新快照为准。</p>`:'';
  chatReplace('#chat-feed',history+(rows.length?rows.map(chatRow).join(''):`<div class="chat-empty"><h2>${chat.loaded?'把第一项任务发到群里':'正在读取对话'}</h2><p>${chat.loaded?'接单、分工、工具动作、进展与结果会在这里持续出现。':'正在读取任务与执行记录。'}</p></div>`),true);
  const form=$('#chat-form'),input=$('#chat-input'),submit=form.querySelector('[type="submit"]');
  input.readOnly=chat.sending||Boolean(chat.pending);submit.disabled=chat.sending;
  submit.textContent=chat.sending?'正在发送…':chat.pending?'重试发送':'发送任务';
  form.setAttribute('aria-busy',String(chat.sending));
  $('#chat-send-error').textContent=chat.sendError||'';
  if(!chat.source&&!chat.loaded&&!chat.connecting)chatConnect();
}
function chatReplace(selector,html,scroll=false){
  const el=$(selector);if(!el||el.chatMarkup===html)return;
  const open=new Set([...el.querySelectorAll('details[open][id]')].map(d=>d.id));
  const focus=document.activeElement,focusId=el.contains(focus)?focus.id:null;
  const action=el.contains(focus)?focus.dataset.action:null,id=focus?.dataset.id;
  const top=el.scrollTop,height=el.scrollHeight;
  const anchor=scroll?[...el.querySelectorAll('.chat-message')].find(row=>row.offsetTop>=top):null;
  const offset=anchor?anchor.getBoundingClientRect().top-el.getBoundingClientRect().top:0;
  el.innerHTML=html;el.chatMarkup=html;
  el.querySelectorAll('details[id]').forEach(d=>{d.open=open.has(d.id);});
  if(scroll){
    const retained=anchor?document.getElementById(anchor.id):null;
    el.scrollTop=chat.follow?el.scrollHeight:retained?el.scrollTop+retained.getBoundingClientRect().top-el.getBoundingClientRect().top-offset:top+el.scrollHeight-height;
  }
  const restore=focusId?document.getElementById(focusId):action?[...el.querySelectorAll('[data-action]')].find(e=>e.dataset.action===action&&e.dataset.id===id):null;
  restore?.focus({preventScroll:true});
}
async function chatConnect(){
  const request=++chat.request,root=chatTaskId();chat.source?.close();chat.source=null;chat.connecting=true;
  chat.status='正在连接';chat.error='';
  try{
    const chunk=await api(`/api/chat/history${root?`?root_id=${encodeURIComponent(root)}`:''}`);
    if(request!==chat.request||page()!=='chat')return;
    chat.events=chunk.events;chat.hasMore=chunk.has_more;chat.loaded=true;chat.follow=true;
    const source=new EventSource(`/api/chat/stream?after=${chunk.cursor}${root?`&root_id=${encodeURIComponent(root)}`:''}`);
    chat.source=source;
    source.addEventListener('open',()=>{if(chat.source!==source)return;chat.status='实时连接已建立';chat.error='';chatRender();});
    source.addEventListener('activity',event=>{
      if(chat.source!==source)return;
      const item=JSON.parse(event.data);
      if(!chat.events.some(e=>e.seq===item.seq&&e.root_id===item.root_id))chat.events.push(item);
      if(chat.follow&&chat.events.length>2000){chat.events.splice(0,chat.events.length-2000);chat.hasMore=true;}
      chat.error='';chatRender();
    });
    source.addEventListener('read-error',()=>{if(chat.source===source){chat.error='执行动态读取失败，正在重试。';chatRender();}});
    source.addEventListener('error',()=>{if(chat.source===source){chat.status='实时连接中断，正在重连';chatRender();}});
    chatRender();
  }catch(error){if(request===chat.request&&page()==='chat'){chat.error=`读取对话失败：${error.message}。点击刷新重试。`;chat.loaded=true;chatRender();}}
  finally{if(request===chat.request)chat.connecting=false;}
}
function chatLeave(){chat.request++;chat.source?.close();chat.source=null;chat.connecting=false;chat.loaded=false;chat.events=[];}
async function chatSend(event){
  event.preventDefault();if(chat.sending)return;
  const text=chat.pending?.text||$('#chat-input').value.trim();
  if(!text||new TextEncoder().encode(text).length>8000){chat.sendError='任务不能为空，最多 8,000 个 UTF-8 字节。';chatRender();$('#chat-send-error').focus();return;}
  chat.pending ||= {text,key:crypto.randomUUID()};chat.sending=true;chat.sendError='';chatRender();
  try{
    const result=await api('/api/board/tasks',{title:Array.from(text).slice(0,50).join(''),instructions:text,acceptance:'完成本次输入中的目标与约束，提供可核对的结果，并说明未完成事项、证据与阻塞原因。'},'POST',chat.pending.key);
    chat.pending=null;chat.draft='';if($('#chat-input'))$('#chat-input').value='';
    await refresh(true);
    if(page()==='chat'){navigate(`/chat/${result.id}`);chat.follow=true;$('#chat-input')?.focus();}
    else toast('任务已发布，蚁群将按现有规则接单。');
  }catch(error){
    const rejected=error.status>=400&&error.status<500;
    chat.sendError=rejected?error.message:`${error.message}。是否已接纳尚未确认，请重试发送；重试会复用同一请求。`;
    if(rejected)chat.pending=null;
  }finally{chat.sending=false;if(page()==='chat'){chatRender();if(chat.sendError)$('#chat-send-error')?.focus();}}
}
async function chatAction(action){
  if(action==='chat-refresh'){chat.loaded=true;await refresh(true);await chatConnect();return true;}
  if(action==='chat-all'){navigate('/chat');return true;}
  if(action==='chat-history'){
    const earliest=chat.events[0]?.seq;if(!earliest)return true;
    const request=chat.request,root=chatTaskId();
    const chunk=await api(`/api/chat/history?before=${earliest}${root?`&root_id=${encodeURIComponent(root)}`:''}`);
    if(request!==chat.request||page()!=='chat')return true;
    const keys=new Set(chat.events.map(e=>`${e.seq}:${e.root_id}`));
    chat.events=[...chunk.events.filter(e=>!keys.has(`${e.seq}:${e.root_id}`)),...chat.events];chat.hasMore=chunk.has_more;chat.follow=false;chatRender();return true;
  }
  return false;
}
