const {test}=require('node:test');
const assert=require('node:assert/strict');
const {readFileSync}=require('node:fs');
const {resolve}=require('node:path');
const vm=require('node:vm');
function fixture(){
  const c=vm.createContext({TextEncoder,location:{pathname:'/chat'},time:()=>'',short:s=>String(s).slice(0,15),fleetId:()=>null});
  const app=readFileSync(resolve(__dirname,'../web/src/app/app.js'),'utf8');
  vm.runInContext(app.slice(0,app.indexOf("document.addEventListener('click'")),c);
  for(const path of ['board/board.js','chat/chat.js'])vm.runInContext(readFileSync(resolve(__dirname,'../web/src/features/'+path),'utf8'),c);
  vm.runInContext(`data={board:{tasks:[{id:'root',root_id:'root',title:'任务',instructions:'<script>任务</script>',state:'running',created_at:1}],agents:[]},instances:[]}`,c);
  return c;
}
test('同一任务的两位成员独立流式呈现，完成帧替换前缀且不重复显示整轮文字',()=>{
  const c=fixture();
  vm.runInContext(`chat.events=[
    {seq:1,at:2,kind:'execution.text_delta',root_id:'root',instance_id:'ant-a',run_id:'run-a',data:{generation:'g1',offset:0,text:'蚂蚁'}},
    {seq:2,at:3,kind:'execution.text_delta',root_id:'root',instance_id:'ant-b',run_id:'run-b',data:{generation:'g2',offset:0,text:'另一位'}},
    {seq:3,at:4,kind:'execution.text_delta',root_id:'root',instance_id:'ant-a',run_id:'run-a',data:{generation:'g1',offset:6,text:'结果'}},
    {seq:4,at:5,kind:'execution.text_end',root_id:'root',instance_id:'ant-a',run_id:'run-a',data:{generation:'g1',text:'蚂蚁结果'}},
    {seq:5,at:6,kind:'execution.model_completed',root_id:'root',instance_id:'ant-a',run_id:'run-a',data:{text:'蚂蚁结果'}}
  ];`,c);
  const rows=vm.runInContext('chatRows()',c);
  assert.equal(rows.length,3);
  assert.equal(rows[1].event.data.text,'蚂蚁结果');
  assert.equal(rows[2].event.data.text,'另一位');
  const html=vm.runInContext('chatRows().map(chatRow).join("")',c);
  assert(html.includes('ant-a'));assert(html.includes('ant-b'));
  assert(!html.includes('<script>'));assert(html.includes('&lt;script&gt;'));
});
test('历史从流中途开始会标记缺失，完整结束帧可恢复全文',()=>{
  const c=fixture();
  vm.runInContext(`chat.events=[{seq:1,at:2,kind:'execution.text_delta',root_id:'root',run_id:'r',data:{generation:'g',offset:12,text:'后半段'}}]`,c);
  assert.equal(vm.runInContext('chatRows()[1].event.data.partial',c),true);
  vm.runInContext(`chat.events.push({seq:2,at:3,kind:'execution.text_end',root_id:'root',run_id:'r',data:{generation:'g',text:'完整全文',error:{code:'cancelled'}}})`,c);
  assert.equal(vm.runInContext('chatRows()[1].event.data.partial',c),false);
  assert(vm.runInContext('chatRows().map(chatRow).join("")',c).includes('本次生成中断'));
});
test('发送结果未知时保留输入与幂等键，重试成功只发布同一请求',async()=>{
  const c=fixture();let attempts=[];
  c.crypto={randomUUID:()=> 'stable-key'};
  c.document={activeElement:{},querySelector:s=>s==='#chat-input'?{value:'新任务'}:{focus(){}}};
  c.api=async(path,body,method,key)=>{attempts.push({path,body,key});if(attempts.length===1)throw new Error('network');return {id:'created'};};
  c.refresh=async()=>{};c.navigate=()=>{};c.toast=()=>{};
  vm.runInContext('chatRender=()=>{}',c);
  await vm.runInContext('chatSend({preventDefault(){}})',c);
  assert.equal(vm.runInContext('chat.pending.key',c),'stable-key');
  assert.equal(vm.runInContext('chat.pending.text',c),'新任务');
  await vm.runInContext('chatSend({preventDefault(){}})',c);
  assert.deepEqual(attempts[0],attempts[1]);
  assert.equal(attempts[0].path,'/api/board/tasks');
  assert.equal(attempts[0].body.instructions,'新任务');
  assert.equal(vm.runInContext('chat.pending',c),null);
});
test('中文任务按 UTF-8 字节校验，超限输入不发布任务',async()=>{
  const c=fixture();let calls=0;
  c.document={querySelector:s=>s==='#chat-input'?{value:'蚂'.repeat(2667)}:{focus(){}}};
  c.api=async()=>{calls++;};vm.runInContext('chatRender=()=>{}',c);
  await vm.runInContext('chatSend({preventDefault(){}})',c);
  assert.equal(calls,0);assert(vm.runInContext('chat.sendError',c).includes('UTF-8'));
});
test('服务端暂时失败时保留请求，明确拒绝后允许编辑',async()=>{
  for(const status of [503,400]){
    const c=fixture();
    c.crypto={randomUUID:()=> 'request-key'};
    c.document={querySelector:s=>s==='#chat-input'?{value:'任务'}:{focus(){}}};
    c.api=async()=>{throw Object.assign(new Error('未完成'),{status});};
    vm.runInContext('chatRender=()=>{}',c);
    await vm.runInContext('chatSend({preventDefault(){}})',c);
    assert.equal(vm.runInContext('chat.pending?.key||null',c),status===503?'request-key':null);
  }
});
