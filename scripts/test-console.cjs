const {test}=require('node:test');
const assert=require('node:assert/strict');
const {readFileSync}=require('node:fs');
const {resolve}=require('node:path');
const vm=require('node:vm');

function consoleContext(){
  const context=vm.createContext({});
  const app=readFileSync(resolve(__dirname,'../web/src/app/app.js'),'utf8');
  vm.runInContext(app.slice(0,app.indexOf("document.addEventListener('click'")),context);
  return context;
}

test('历史记录包含日期，跨日运行不会只显示相同时刻',()=>{
  const context=consoleContext();
  const values=vm.runInContext("[time('2026-10-28T08:00:00Z'),time('2026-10-29T08:00:00Z'),time(null)]",context);
  assert.notEqual(values[0],values[1]);
  assert.equal(values[2],'未记录');
});

test('能力列表按涌现路径分组，每项仅有一个状态标签，范围与验证信息完整保留',()=>{
  const context=consoleContext();
  vm.runInContext(`data={capabilities:[{name:'能力 <A>',phase:'P1',path:'emergence',status:'partial',scope:'extension',verification:'native-tests-and-browser',detail:'已知限制',source:['crates/ant3-core/src/lib.rs'],target:['future/']}]}`,context);
  const html=vm.runInContext('progressPage()',context);
  assert.equal((html.match(/class="badge /g)||[]).length,1);
  assert(html.includes('涌现机制'));
  assert(html.includes('后续扩展'));
  assert(html.includes('原生测试与 Web 联调'));
  assert(html.includes('crates/ant3-core/src/lib.rs'));
  assert(html.includes('能力 &lt;A&gt;'));
});

test('仅操作列固定在表格右侧，数据列正常滚动',()=>{
  const context=consoleContext();
  assert(vm.runInContext("table(['目标','操作'],[])",context).includes('class="table-actions"'));
  assert(!vm.runInContext("table(['目标','时间'],[])",context).includes('class="table-actions"'));
});

test('总览优先显示忙碌实例，不将已终止实例列入未终止列表',()=>{
  const context=consoleContext();
  vm.runInContext(`data={
    instances:[{id:'old',preset_name:'已终止身份',state:'terminated',snapshot:{protocol:'demo'}},
      {id:'ready',preset_name:'就绪身份',state:'ready',snapshot:{protocol:'demo'}},
      {id:'busy',preset_name:'忙碌身份',state:'busy',snapshot:{protocol:'demo'}}],
    board:{tasks:[{id:'root',root_id:'root',title:'待评审任务',state:'completed',review_state:'pending'}]},runs:[],presets:[],models:[],limits:{workers:8}
  }`,context);
  const html=vm.runInContext('overview()',context);
  assert(!html.includes('已终止身份'));
  assert(html.indexOf('忙碌身份')<html.indexOf('就绪身份'));
  assert(html.includes('待评审任务'));
});

test('插件目录只显示内置能力，不提供安装入口，并明确未配置状态',()=>{
  const context=consoleContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/plugins/plugins.js'),'utf8'),context);
  vm.runInContext(`data={builtin_plugins:[{id:'web.search',version:'1.0.0',description:'搜索',tools:[]}],builtin_plugin_status:{'web.search':{configured:false}}}`,context);
  const html=vm.runInContext('pluginsPage()',context);
  assert(html.includes('web.search'));assert(html.includes('未配置'));
  assert(!html.includes('安装'));assert(!html.includes('process-form'));assert(!html.includes('plugin-form'));
});

test('详情中的插件授权也服从全局执行开关',()=>{
  const context=consoleContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/plugins/plugins.js'),'utf8'),context);
  vm.runInContext(`data={process_plugin_execution:false,presets:[]};
    let detail='';openModal=(title,html)=>{detail=html};
    processDetail({id:'local.test',version:'1',state:'enabled',definition:{manifest:{runtime:{},tools:[]}}});`,context);
  const html=vm.runInContext('detail',context);
  assert(html.includes('全局执行已关闭'));
  assert(!html.includes('允许调用'));
});

test('离开页面或关闭后，过期的产物与文件请求不再打开弹窗',async()=>{
  for(const call of ["workspaceOpen('id')","workspaceFile('id','file.txt')","workspaceArtifactDetail('id')"]){
    const context=consoleContext();
    context.document={addEventListener(){}};
    vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/workspaces/workspaces.js'),'utf8'),context);
    vm.runInContext(`let finish;api=()=>new Promise(resolve=>{finish=resolve});
      openModal=()=>{throw new Error('stale dialog opened')};`,context);
    const pending=vm.runInContext(call,context);
    vm.runInContext('inspectionRequest++;finish({})',context);
    await pending;
  }
});

test('母体独立定义默认选择可用协议，继承切换恢复必填状态并清除过期错误',()=>{
  const context=consoleContext();
  context.document={addEventListener(){},querySelector(){return null}};
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/presets/presets.js'),'utf8'),context);
  vm.runInContext(`
    data={presets:[{name:'parent',version:'1',definition:{purpose:'test',plugin_snapshots:[]}}]};
    const summary={};document.querySelector=()=>summary;
    const field=()=>({required:true,error:'old',setCustomValidity(v){this.error=v}});
    const form={elements:{extends:{value:''},purpose:field(),objective:field(),instructions:field(),protocol:{...field(),value:'',options:[{value:''},{value:'demo'}]}}};
    updatePresetParent(form);
  `,context);
  assert.equal(vm.runInContext('form.elements.protocol.value',context),'demo');
  assert.equal(vm.runInContext('form.elements.protocol.required',context),true);
  vm.runInContext(`form.elements.extends.value=JSON.stringify({name:'parent',version:'1'});updatePresetParent(form)`,context);
  assert.equal(vm.runInContext('form.elements.protocol.value',context),'');
  assert.equal(vm.runInContext('form.elements.purpose.required',context),false);
  assert.equal(vm.runInContext('form.elements.purpose.error',context),'');
  vm.runInContext(`form.elements.extends.value='';updatePresetParent(form)`,context);
  assert.equal(vm.runInContext('form.elements.protocol.value',context),'demo');
  assert.equal(vm.runInContext('form.elements.objective.required',context),true);
});

test('标识格式兼容浏览器 Unicode sets 正则，并拒绝中文和空格',()=>{
  const context=consoleContext();context.document={addEventListener(){},querySelector(){return null}};
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/presets/presets.js'),'utf8'),context);
  vm.runInContext(`data={presets:[],plugins:[],models:[]};let markup='';openModal=(title,html)=>{markup=html};updatePresetParent=()=>{};presetForm()`,context);
  const html=vm.runInContext('markup',context),pattern=html.match(/pattern="([^"]+)"/)[1];
  const re=new RegExp(`^(?:${pattern})$`,'v');
  assert(re.test('researcher-1.0_test'));
  assert(!re.test('研究员'));assert(!re.test('research assistant'));
});

// —— 观察者视角：默认页 / 重定向 / 群体翻译 / 待处理聚合 ——

function featureContext(extra={}){
  const context=consoleContext();
  context.document={addEventListener(){},querySelector(){return null}};
  context.queueMicrotask=()=>{};
  Object.assign(context,extra);
  return context;
}

test('默认首页是工作现场，旧 hash 全部重定向到新导航',()=>{
  const context=featureContext();
  context.location={hash:''};
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/board/board.js'),'utf8'),context);
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/agents/fleets.js'),'utf8'),context);
  assert.equal(vm.runInContext('page()',context),'studio');
  for(const [old,next] of Object.entries({overview:'studio',agents:'researcher',fleets:'researcher',messages:'researcher',runs:'researcher',events:'researcher',capabilities:'researcher',workspaces:'deliveries'})){
    context.location.hash=`#${old}`;
    assert.equal(vm.runInContext('page()',context),next,`${old} → ${next}`);
  }
  for(const hash of ['studio','colony','deliveries','board','inbox','presets','plugins','researcher']){
    context.location.hash=`#${hash}`;
    assert.equal(vm.runInContext('page()',context),hash);
  }
  context.location.hash='#board/task-1';
  assert.equal(vm.runInContext('page()',context),'board');
  context.location.hash='#fleet/f1';
  assert.equal(vm.runInContext('page()',context),'researcher');
});

test('群体事件翻译成一句话：认领 / 拆分 / 参考 / 质检 / 人类评审',()=>{
  const context=featureContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/colony/colony.js'),'utf8'),context);
  const snapshot={instances:[{id:'agent-9527',preset_name:'analyst',state:'busy'}],board:{tasks:[{id:'task-abcdef12',title:'检查数据'}]}};
  context.snapshot=snapshot;
  const say=(kind,subject,d)=>vm.runInContext(`swarmNarrative(${JSON.stringify({kind,subject,data:d})},snapshot)`,context);
  assert.equal(say('swarm.self_selected','task-abcdef12',{preset_name:'analyst'}),'一只analyst主动接了「检查数据」');
  assert.equal(say('board.task_claimed','task-abcdef12',{instance_id:'agent-9527'}),'analyst #9527开始处理「检查数据」');
  assert.equal(say('board.task_split','task-abcdef12',{parent_id:'task-abcdef12',task_ids:['a','b','c']}),'「检查数据」被拆成3份独立工作');
  assert.equal(say('board.trace_deposited','task-abcdef12',{reason:'reuse'}),'一份成果被同伴参考');
  assert.equal(say('board.trace_deposited','task-abcdef12',{reason:'failure'}),'一条路径失败，群体将换方向');
  assert.equal(say('board.audit_settled','task-abcdef12',{state:'inconclusive'}),'质检证据不足，交人类定夺');
  assert.equal(say('board.task_reviewed','task-abcdef12',{author_kind:'operator',decision:'accepted'}),'人类评审：通过');
  assert.equal(say('board.task_reviewed','task-abcdef12',{author_kind:'verifier',decision:'changes_requested'}),'质检退回，附修改意见');
  assert.equal(say('board.rework_cooldown','task-abcdef12',{chain:3,cooldown_ms:60000}),'反复修改仍未达标，暂缓重试');
  assert.equal(say('swarm.colony_starved','colony',{}),'群体预算耗尽，成员休息');
  assert.equal(say('board.energy_posted','colony',{kind:'idle_burn'}),null);
  assert.equal(say('run.started','r',{}),null);
  // 查不到任务时使用短 id，不抛异常。
  assert(say('board.task_claimed','unknown-task-id',{instance_id:'x'}).includes('unknown-'));
});

test('inbox 聚合人类决策点：待验收根任务、质检定论、预算耗尽、结果未知、模型未配置',()=>{
  const context=featureContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/inbox/inbox.js'),'utf8'),context);
  vm.runInContext(`data={
    board:{tasks:[
      {id:'r1',title:'成果A',state:'completed',review_state:'pending',run_id:'run-1'},
      {id:'r2',title:'成果B',state:'completed',review_state:'pending',audits:[{state:'running'}]},
      {id:'r3',title:'成果C',state:'completed',review_state:'pending',audits:[{state:'inconclusive'}]},
      {id:'r4',title:'成果D',state:'failed',error:'energy_exhausted'},
      {id:'c1',parent_id:'r1',title:'子任务',state:'completed',review_state:'pending'},
    ]},
    runs:[{id:'run-x',state:'interrupted'},{id:'run-ok',state:'completed'}],
    models:[{protocol:'demo',configured:false}],
  }`,context);
  const items=vm.runInContext('inboxItems()',context);
  // r1 待验收；r2 质检进行中不打扰；r3 待验收 + 质检定论；r4 预算耗尽；子任务不算；run-x；模型。
  assert.equal(items.length,6);
  assert.deepEqual([...items.map(i=>i.kind)],['review','review','audit','energy','run','models']);
  const html=vm.runInContext('inboxPage()',context);
  assert(html.includes('成果等你验收'));
  assert(html.includes('board-review'));
  assert(!html.includes('子任务'));
});

test('一切正常时 inbox 显示空态，不产生卡片',()=>{
  const context=featureContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/inbox/inbox.js'),'utf8'),context);
  vm.runInContext(`data={board:{tasks:[{id:'r',title:'已交付',state:'completed',review_state:'accepted'}]},runs:[],models:[{protocol:'demo',configured:true}]}`,context);
  assert.equal(vm.runInContext('inboxItems().length',context),0);
  assert(vm.runInContext('inboxPage()',context).includes('群体一切正常，无需你介入。'));
});

test('蚁群花名册合并实例与经验，状态用人话，正在做的任务可追溯',()=>{
  const context=featureContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/colony/colony.js'),'utf8'),context);
  vm.runInContext(`
    colonyPool={enabled:true,peers:[{instance_id:'i-0001',preset_name:'analyst',preset_version:'1',state:'busy'}],pool_max:4,colony_budget:1000,colony_spent:200,idle_burn_per_min:5};
    colonyExperience={experience:[{preset_name:'analyst',preset_version:'1',successes:3,failures:1,energy_spent:10,cost_efficiency:0.5,response_threshold:20}]};
    data={instances:[{id:'i-0001',preset_name:'analyst',preset_version:'1',state:'busy'},{id:'i-0002',preset_name:'gone',preset_version:'1',state:'terminated'}],
      presets:[{name:'analyst',version:'1',definition:{purpose:'擅长数据分析'}}],
      board:{tasks:[{id:'t1',title:'检查数据',state:'running',instance_id:'i-0001'}],agents:[{instance_id:'i-0001',claimed_task_id:'t1'}]}};
  `,context);
  const rows=vm.runInContext('colonyMemberRows()',context);
  assert.equal(rows.length,1);
  assert.equal(rows[0].nickname,'analyst #0001');
  assert.equal(rows[0].purpose,'擅长数据分析');
  assert.equal(rows[0].state,'正在干活');
  assert.equal(rows[0].rate,'75%');
  assert.equal(rows[0].task,'检查数据');
});

test('蚁群页默认文案不出现机制词汇，参数移到研究者页',()=>{
  const context=consoleContext();
  context.queueMicrotask=()=>{};
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/colony/colony.js'),'utf8'),context);
  vm.runInContext("document={addEventListener(){}}",context);
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/researcher/researcher.js'),'utf8'),context);
  vm.runInContext("data={board:{tasks:[],agents:[]},instances:[],presets:[],events:[]}",context);
  vm.runInContext("colonyPool={enabled:true,pool_max:8,peers:[],colony_budget:1000,colony_spent:200,idle_burn_per_min:20}",context);
  vm.runInContext("colonyExperience={experience:[{preset_name:'reader',preset_version:'1',successes:2,failures:1,energy_spent:300,cost_efficiency:0.4,response_threshold:0.31}]}",context);
  const html=vm.runInContext('colonyPage()',context);
  assert(!html.includes('response_threshold'));
  assert(!html.includes('浓度'));
  assert(!html.includes('<details class="researcher"'));
  assert(html.includes('群体总预算还剩 800 / 1,000'));
  // 研究者页承接参数。
  const researcher=vm.runInContext('researcherSwarmSection()',context);
  assert(researcher.includes('响应阈值'));
  assert(researcher.includes('能量效率'));
  assert(researcher.includes('0.31'));
});

test('成果库以交付物为中心，私有文件浏览退到折叠区',()=>{
  const context=featureContext();
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/workspaces/workspaces.js'),'utf8'),context);
  vm.runInContext(`
    workspaceCatalog={configured:true,instances:[{id:'i1',preset_name:'analyst',state:'ready',artifact_count:1}]};
    workspacePublications={artifacts:[{id:'a1',name:'报告',kind:'json',instance_id:'i1',run_id:'r1',source_state:'completed',bytes:10}]};
    data={board:{tasks:[
      {id:'t1',title:'已交付工作',state:'completed',review_state:'accepted',preset_name:'analyst',reuse_count:2},
      {id:'t2',title:'进行中工作',state:'running'},
    ]}};
  `,context);
  const html=vm.runInContext('deliveriesPage()',context);
  const [visible,researcher]=html.split('<details class="researcher"');
  assert(visible.includes('已交付工作'));
  assert(visible.includes('已交付'));
  assert(visible.includes('被参考 2 次'));
  assert(!visible.includes('进行中工作'));
  assert(visible.includes('查看快照'));
  assert(researcher.includes('浏览文件'));
});

test('研究者视图默认全收起且惰性渲染，展开状态由分节集合驱动',()=>{
  const context=featureContext();
  vm.runInContext("document={addEventListener(){}}",context);
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/researcher/researcher.js'),'utf8'),context);
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/agents/fleets.js'),'utf8'),context);
  vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/messages/messages.js'),'utf8'),context);
  context.location={hash:'#researcher'};
  vm.runInContext(`data={instances:[],runs:[],fleets:[],messages:[],capabilities:[],capacity:{reserved:0,limit:8,active:0}};events=[];`,context);
  const html=vm.runInContext('researcherPage()',context);
  assert(html.includes('研究者视图：机制参数与原始记录'));
  assert(!html.includes(' open'), '默认全部收起');
  assert(!html.includes('directory-search'), '收起的分节不渲染控件，避免重复 ID');
  vm.runInContext("researcherSections.add('runs')",context);
  const opened=vm.runInContext('researcherPage()',context);
  assert.equal((opened.match(/id="directory-search"/g)||[]).length,1);
});
