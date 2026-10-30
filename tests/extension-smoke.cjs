// Run: node tests/extension-smoke.cjs (no browser or system changes).
const fs=require('fs'),vm=require('vm'),assert=require('assert');
async function test(flavor){
 const elements=new Map(),messages=[];
 const element=id=>{if(!elements.has(id))elements.set(id,{textContent:'',disabled:false,classList:{toggle(){}},events:{},addEventListener(name,fn){this.events[name]=fn;}});return elements.get(id);};
 let state='none';const reply=message=>{messages.push(message);if(message.action==='general'||message.action==='exclude')state=message.action;return {ok:true,state};};
 const api={tabs:{query(query,cb){const result=[{url:'https://sub.example.com:8443/path?q=private'}];if(cb)cb(result);else return Promise.resolve(result);}},runtime:{sendNativeMessage(host,message,cb){assert.equal(host,'com.zapret_gui.lists');const result=reply(message);if(cb)cb(result);else return Promise.resolve(result);}}};
 const context={document:{getElementById:element},URL,console};context[flavor==='firefox'?'browser':'chrome']=api;
 const manifest=JSON.parse(fs.readFileSync(`extensions/${flavor}/manifest.json`,'utf8'));assert.deepEqual(manifest.permissions,['activeTab','nativeMessaging']);
 vm.runInNewContext(fs.readFileSync(`extensions/${flavor}/popup.js`,'utf8'),context);
 await new Promise(resolve=>setImmediate(resolve));
 assert.equal(messages[0].action,'status');assert.equal(messages[0].domain,'sub.example.com');assert.equal(element('general').disabled,false);
 await element('general').events.click();assert.match(element('state').textContent,/list-general-user/);
 await element('exclude').events.click();assert.match(element('state').textContent,/list-exclude-user/);
 await element('open').events.click();assert.equal(messages.at(-1).action,'open_gui');
 console.log(flavor+': domain extraction, status, both actions, open GUI — passed');
}
(async()=>{await test('chromium');await test('firefox');})().catch(e=>{console.error(e);process.exit(1);});
