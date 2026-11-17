const {test}=require('node:test');
const assert=require('node:assert/strict');
const {readFileSync}=require('node:fs');
const {resolve}=require('node:path');
const vm=require('node:vm');
const context=vm.createContext({location:{hash:'#board'},time:()=>'',short:value=>String(value??'').slice(0,15)});
Object.assign(context,{
  esc:(value='')=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])),
  badge:(state,text)=>`<span>${text}</span>`,button:(label,action,primary,attributes='')=>`<button data-action="${action}" ${attributes}>${label}</button>`,
  head:(_,title,description,actions)=>`<h1>${title}</h1><p>${description}</p>${actions}`,
  panel:(title,body,extra='')=>`<section><h2>${title}</h2>${extra}${body}</section>`,
  empty:(title,description,action='')=>`${title}${description}${action}`,
  formEnd:label=>`<button type="submit">${label}</button>`,
  openModal:(title,html)=>{context.form=html;},dataView:value=>JSON.stringify(value),
});
vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/board/board.js'),'utf8'),context);

test('发布表单只收集任务；执行 Preset 不可选，验证器为可选细节',()=>{
  context.data={presets:[{name:'auditor',version:'1'}]};
  vm.runInContext('boardPublishForm()',context);
  assert(context.form.includes('name="instructions"'));
  assert(context.form.includes('name="acceptance"'));
  assert(!context.form.includes('name="preset"'));
  assert(!context.form.includes('name="instance_id"'));
  // 可选独立验证直接平铺，默认不启用。
  assert(context.form.includes('独立验证（可选）'));
  assert(!context.form.includes('<details'));
  assert(context.form.includes('value="">不启用（仅人类评审）'));
  assert(context.form.includes('name="verify_preset"'));
});

test('空任务板提供发布入口，快照尚无 board 时可正常显示',()=>{
  context.data={};
  const html=vm.runInContext('boardPage()',context);
  assert(html.includes('还没有发布任务'));
  assert.equal((html.match(/data-action="board-publish"/g)||[]).length,1);
  assert(html.includes('0 项待完成'));
});

test('评审通过与取消归入历史，待评审、失败和结果未知任务保留在当前列表',()=>{
  context.location.hash='#board';
  context.data={board:{tasks:[
    {id:'done',title:'已交付',state:'completed',review_state:'accepted'},
    {id:'review',title:'等待人类检查',state:'completed',review_state:'pending'},
    {id:'cancel',title:'已撤回',state:'cancelled'},
    {id:'failed',title:'需要排查',state:'failed',instructions:'<img src=x>检查失败原因'},
    {id:'unknown',title:'需要核对',state:'interrupted'},
    {id:'blocked',title:'需要解除阻断',state:'blocked'},
    {id:'pending',title:'等待执行',state:'pending'},
  ]}};
  const html=vm.runInContext('boardPage()',context),[current,history]=html.split('历史任务');
  for(const title of ['需要排查','需要核对','需要解除阻断','等待执行','等待人类检查'])assert(current.includes(title));
  assert(current.includes('等你验收'));
  assert(!current.includes('已交付'));
  assert(history.includes('已交付'));
  assert(history.includes('已撤回'));
  assert(!html.includes('<img'));
  assert(html.includes('&lt;img src=x&gt;'));
});

test('成果、进度和人类评审意见可见且转义，评审绑定当前 Run',()=>{
  context.location.hash='#board/review';
  context.data={board:{tasks:[{id:'review',title:'检查',state:'completed',review_state:'pending',run_id:'run-current',result:{output:'<img src=x>成果'},progress:{message:'<script>进度</script>'},reviews:[{decision:'changes_requested',feedback:'<b>补充证据</b>',run_id:'run-previous'}]}]}};
  const html=vm.runInContext('boardPage()',context);
  assert(html.includes('等你验收'));
  assert(html.includes('board-review'));
  assert(html.includes('data-run="run-current"'));
  assert(html.includes('&lt;b&gt;补充证据&lt;/b&gt;'));
  assert(!html.includes('<img'));
  assert(!html.includes('<script>'));
});

test('已安排未开工和执行中状态分开显示，全部使用人话',()=>{
  context.task={state:'pending'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'等待接活');
  context.task.preset_name='analyst';
  assert.equal(vm.runInContext('boardStatus(task)',context),'等待开工');
  context.task.state='running';
  assert.equal(vm.runInContext('boardStatus(task)',context),'进行中');
  context.task={state:'completed',review_state:'pending'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'等你验收');
  context.task={state:'completed',review_state:'accepted'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'已交付');
  context.task={state:'pending',review_state:'changes_requested'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'按意见修改中');
  context.task={state:'failed',error:'energy_exhausted'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'预算耗尽');
});

test('任务详情显示拆分来源与子任务，同时转义任务内容',()=>{
  context.location.hash='#board/root';
  context.data={board:{tasks:[
    {id:'root',root_id:'root',title:'<script>bad</script>',instructions:'<img src=x>',acceptance:'完成',state:'waiting'},
    {id:'child',root_id:'root',parent_id:'root',title:'子任务',state:'pending'},
  ]}};
  const html=vm.runInContext('boardPage()',context);
  assert(!html.includes('<script>'));
  assert(!html.includes('<img'));
  assert(html.includes('子任务'));
  assert(html.includes('board-cancel'));
  context.location.hash='#board/child';
  assert(vm.runInContext('boardPage()',context).includes('拆分来源'));
});

test('父任务重新安排时不把历史 preset 显示成已分配',()=>{
  context.task={state:'pending',preset_name:'old-preset',route_state:'unassigned'};
  assert.equal(vm.runInContext('boardStatus(task)',context),'等待接活');
  assert.equal(vm.runInContext('boardPreset(task)',context),null);
});

test('安排受阻显示人话原因，原始错误码不出现在用户文案',()=>{
  context.location.hash='#board';
  context.data={board:{tasks:[{id:'retry',title:'待重试任务',state:'pending',route_state:'unassigned'}],routing:[{task_id:'retry',state:'waiting',error:'router_unavailable',attempts:2}]}};
  const html=vm.runInContext('boardPage()',context);
  assert(html.includes('安排受阻，稍后重试'));
  assert(!html.includes('router_unavailable'));
});

test('列表行用人话徽章展示被参考次数，不显示浓度徽章',()=>{
  context.location.hash='#board';
  context.data={board:{tasks:[
    {id:'reused',title:'被参考的工作',state:'running',reuse_count:3,trace:{kind:'result',strength:88}},
    {id:'plain',title:'普通工作',state:'running',trace:{kind:'result',strength:12}},
  ]}};
  const html=vm.runInContext('boardPage()',context);
  assert(html.includes('被参考 3 次'));
  assert(!html.includes('trace-badge'));
  assert(!html.includes('浓度'));
});

test('任务详情只讲结果，机制数据全部在研究者页',()=>{
  context.location.hash='#board/root';
  context.data={board:{tasks:[{id:'root',root_id:'root',title:'工作',instructions:'目标',acceptance:'标准',state:'running',route_reason:'原始选型依据',trace:{kind:'result',strength:66},reuse_count:2,energy:{budget:100000,spent:4000,refunded:0,available:96000}}]}};
  const html=vm.runInContext('boardPage()',context);
  assert(!html.includes('<details class="researcher"'),'任务板不再内嵌研究者折叠');
  assert(!html.includes('浓度'));
  assert(!html.includes('阈值'));
  assert(!html.includes('账本'));
  assert(!html.includes('半衰期'));
  assert(html.includes('被同伴参考 2 次'));
  assert(html.includes('花费'));
});

