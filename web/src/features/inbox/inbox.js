'use strict';

// 聚合所有需要人类决策的事项。每张卡片对应一个待处置点。
function inboxAuditReason(code){
  return ({audit_run_incomplete:'质检过程意外中断',audit_deadline_exceeded:'质检超时',audit_instance_unavailable:'没有可用的质检成员',audit_instance_not_fresh:'质检成员不独立',audit_context_too_large:'成果过长无法质检',board_result_changed:'成果在质检期间被改动',board_task_reopened:'任务已重新开放'})[code]||'质检异常';
}

function inboxItems(snapshot=data){
  const items=[],tasks=snapshot?.board?.tasks||[],runs=snapshot?.runs||[],models=snapshot?.models||[];
  for(const task of tasks){
    const audits=task.audits||[],latest=audits[audits.length-1];
    const auditing=latest&&['pending','running'].includes(latest.state);
    if(!task.parent_id&&task.review_state==='pending'&&!auditing)
      items.push({kind:'review',id:task.id,title:'成果等你验收',text:`「${task.title}」已完成，等待你按验收标准检查。`,task});
    if(latest&&['inconclusive','failed'].includes(latest.state))
      items.push({kind:'audit',id:task.id,title:'质检无法定论，需要你判断',text:`「${task.title}」的独立质检未能给出结论${latest.error?`（${inboxAuditReason(latest.error)}）`:''}，请人工核对成果。`,task});
    if(task.state==='failed'&&task.error==='energy_exhausted')
      items.push({kind:'energy',id:task.id,title:'预算耗尽停止',text:`「${task.title}」因预算耗尽停止。暂不支持在线追加预算；可取消后以更高预算重新发布。`,task});
  }
  for(const run of runs)if(run.state==='interrupted')
    items.push({kind:'run',id:run.id,title:'有执行结果未知，需处置',text:`一次执行（${String(run.id).slice(0,15)}）中断且结果未知，请在研究者视图核对执行记录后处置。`,run});
  if(models.length&&!models.some(m=>m.configured))
    items.push({kind:'models',id:'models',title:'先配置模型',text:'尚未配置任何模型凭证，群体无法真正开始工作。请配置模型并重启服务。'});
  return items;
}
function inboxCard(item){
  let actions='';
  if(item.kind==='review')actions=`<a class="button" href="#board/${esc(item.id)}">查看成果</a>${button('评审','board-review',true,`data-id="${esc(item.id)}" data-run="${esc(item.task?.run_id||'')}"`)}`;
  if(item.kind==='audit')actions=`<a class="button" href="#board/${esc(item.id)}">查看任务与质检结论</a>${item.task?.review_state==='pending'?button('人工评审','board-review',true,`data-id="${esc(item.id)}" data-run="${esc(item.task?.run_id||'')}"`):''}`;
  if(item.kind==='energy')actions=`<span class="hint">追加预算功能即将提供。</span>${button('取消任务','board-cancel',false,`data-id="${esc(item.id)}"`)}`;
  if(item.kind==='run')actions=`<a class="button" href="#researcher">到研究者视图处置</a>${button('执行详情','run-detail',false,`data-id="${esc(item.id)}"`)}`;
  return `<div class="work-item inbox-card inbox-${esc(item.kind)}"><div class="work-item-head"><div><h3>${esc(item.title)}</h3></div>${badge('awaiting-review','需要你')}</div><div class="work-item-meta"><span class="record-text">${esc(item.text)}</span></div><div class="task-form-actions">${actions}</div></div>`;
}
function inboxPage(){
  const items=inboxItems();
  return head('需要你处理')+
    (items.length?`<div class="work-items">${items.map(inboxCard).join('')}</div>`:empty('群体一切正常，无需你介入。','成员们正在自行协作；出现需要人类决策的事项时会在这里提醒你。'));
}