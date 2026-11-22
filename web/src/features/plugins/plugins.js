'use strict';
function pluginsPage() {
  const rows=(data.builtin_plugins||[]).map(p=>{
    const configured=data.builtin_plugin_status?.[p.id]?.configured!==false;
    return `<tr><td><div class="primary-text">${esc(p.id)}</div><div class="secondary-text mono">${esc(p.version)}</div></td><td>${esc(p.description)}</td><td>${badge(configured?'good':'neutral',configured?'可用':'未配置')}</td><td>${(p.tools||[]).length}</td><td><button class="text-button" data-action="builtin-detail" data-id="${esc(p.id)}">查看能力</button></td></tr>`;
  });
  const processes=data.process_plugins||[];
  const processRows=processes.map(p=>{
    const enabled=p.state==='enabled';
    return `<tr><td><div class="primary-text">${esc(p.id)}</div><div class="secondary-text mono">${esc(p.version)}</div></td><td>${badge(enabled?'good':'neutral',enabled?'允许调用':'已禁用')}</td><td>${p.health_error?'<span class="badge failed">需要检查</span>':'<span class="hint">已通过</span>'}</td><td><button class="text-button" data-action="process-detail" data-id="${esc(p.id)}" data-version="${esc(p.version)}">查看契约</button><button class="text-button" data-action="process-check" data-id="${esc(p.id)}" data-version="${esc(p.version)}">健康检查</button><button class="text-button" data-action="process-state" data-id="${esc(p.id)}" data-version="${esc(p.version)}">${enabled?'禁用':'启用'}</button></td></tr>`;
  });
  const execution=data.process_plugin_execution;
  return head('插件与能力', button('安装进程插件','process-install'))+
    panel('内置插件',rows.length?table(['插件 / 版本','能力','状态','工具','操作'],rows):empty('暂无内置插件',''))+
    panel('进程插件',processRows.length?table(['插件 / 版本','调用状态','健康检查','操作'],processRows):empty('尚未安装进程插件','安装本地包后，需要以 --allow-process-plugins 启动 daemon，实例才能调用它。',button('安装本地包','process-install')),`<span class="board-count">${processes.length} 个版本${execution?'':' · 全局执行已关闭'}</span>`);
}
function builtinDetail(p) {
  const configured=data.builtin_plugin_status?.[p.id]?.configured!==false;
  const tools=p.tools||[];
  openModal(p.id,`<div class="inspector-summary"><div><span>版本</span><strong>${esc(p.version)}</strong></div><div><span>状态</span>${badge(configured?'good':'neutral',configured?'可用':'未配置')}</div></div><dl class="detail-kv"><dt>能力</dt><dd>${esc(p.description)}</dd><dt>绑定</dt><dd>发布 Preset 时选择此插件。</dd></dl>${panel('工具',tools.map(t=>`<div class="panel-body"><strong class="mono">${esc(t.name)}</strong><p class="record-text">${esc(t.description)}</p><div class="schema-block"><span class="form-section-title">可填参数</span>${schemaSummary(t.parameters)}</div></div>`).join(''))}`);
}
function processDetail(p) {
  const manifest=p.definition.manifest;
  const bound=data.presets.filter(item=>presetBindings(item.definition).some(b=>b.kind==='process'&&b.id===p.id&&b.version===p.version));
  const availability=badge(data.process_plugin_execution&&p.state==='enabled'?'good':'neutral',!data.process_plugin_execution?'全局执行已关闭':p.state==='enabled'?'允许调用':'已禁用');
  openModal(`${p.id} @ ${p.version}`,`<div class="inspector-summary"><div><span>调用状态</span>${availability}</div><div><span>历史健康检查</span><strong>${p.health_error?'失败':'已通过'}</strong></div><div><span>当前列表中的 Preset 绑定</span><strong>${bound.length}</strong></div></div><p class="form-intro">${esc(manifest.description)}</p>${identity('包摘要',p.digest)}${p.health_error?`<div class="callout">${esc(p.health_error)}。检查包缓存和入口，健康检查通过后再启用。</div>`:''}<dl class="detail-kv"><dt>协议</dt><dd class="mono">${esc(manifest.runtime.protocol)}</dd><dt>包内入口</dt><dd class="mono">${esc(manifest.runtime.entry)}</dd><dt>调用期限</dt><dd>${manifest.timeout_ms} ms；取消后最多额外等待 250 ms 回执</dd><dt>指令</dt><dd class="instructions">${esc(manifest.instructions)}</dd><dt>Preset 绑定</dt><dd>${bound.map(b=>`${esc(b.name)} @ ${esc(b.version)}`).join('<br>')||'当前列表未发现绑定'}</dd></dl>${panel('工具契约',table(['工具','副作用','输入 / 输出'],manifest.tools.map(t=>`<tr><td><strong class="mono">${esc(p.id)}.${esc(t.name)}</strong><p class="hint">${esc(t.description)}</p></td><td>${t.mutates?'声明有副作用':'声明只读'}<div class="secondary-text">${t.parallel_safe?'允许安全并行':'串行边界'}</div></td><td><div class="schema-block"><span class="form-section-title">可填参数</span>${schemaSummary(t.input_schema)}</div><div class="schema-block"><span class="form-section-title">返回内容</span>${schemaSummary(t.output_schema)}</div></td></tr>`)))}`,'进程插件版本');
}
async function processAction(action,element) {
  if(action==='builtin-detail'){const p=(data.builtin_plugins||[]).find(p=>p.id===element.dataset.id);if(p)builtinDetail(p);return true;}
  if(action==='process-install'){
    openModal('安装本地进程插件',`<form data-form="process-install"><label>包目录（绝对路径）<input name="source_dir" required placeholder="/Users/you/plugins/my-tool"></label><p class="hint">读取目录中的 manifest.json，校验工具契约、相对路径与文件摘要后安装；不会执行包内代码。安装后还需要以 --allow-process-plugins 启动 daemon 才可调用。</p>${formEnd('安装版本')}</form>`,'进程插件');
    return true;
  }
  if(!['process-detail','process-check','process-state'].includes(action))return false;
  const p=(data.process_plugins||[]).find(p=>p.id===element.dataset.id&&p.version===element.dataset.version);
  if(!p)throw new Error('版本不在当前列表中，请刷新后重试。');
  if(action==='process-detail'){processDetail(p);return true;}
  if(action==='process-state') {
    const disable=p.state==='enabled';
    openModal(disable?'禁用这个版本？':'启用这个版本？',`<form data-form="process-state" data-id="${esc(p.id)}" data-version="${esc(p.version)}" data-state="${disable?'disable':'enable'}"><p class="form-intro">${disable?'立即阻止所有已绑定实例的新调用，并请求取消活动调用。已经发生的外部效果不会撤销；中断回执可能标记结果未知。':'先重新校验包摘要和进程健康，再允许已绑定实例调用。旧版本绑定与会话保持不变。'}</p>${identity('插件版本',`${p.id} @ ${p.version}`)}${formEnd(disable?'确认禁用':'检查并启用')}</form>`,'插件调用授权');return true;
  }
  await api(`/api/v1/process-plugins/${encodeURIComponent(p.id)}/${encodeURIComponent(p.version)}/actions`,{action:'check'});await refresh();toast('健康检查通过');return true;
}
