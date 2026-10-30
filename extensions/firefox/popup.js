"use strict";
const HOST = "com.zapret_gui.lists";
const firefox = typeof browser !== "undefined";
const api = firefox ? browser : chrome;
const $ = id => document.getElementById(id);
let domain = "", busy = false;
function controls() { for (const id of ["general", "exclude", "refresh"]) $(id).disabled = busy || !domain; $("open").disabled = busy; }
function native(message) {
  if (firefox) return api.runtime.sendNativeMessage(HOST, message);
  return new Promise((resolve, reject) => api.runtime.sendNativeMessage(HOST, message, response => {
    const error = api.runtime.lastError;
    if (error) reject(new Error(error.message)); else resolve(response);
  }));
}
function tabs() {
  if (firefox) return api.tabs.query({active:true,currentWindow:true});
  return new Promise((resolve,reject)=>api.tabs.query({active:true,currentWindow:true}, result=>{
    const error=api.runtime.lastError;if(error)reject(new Error(error.message));else resolve(result);
  }));
}
function show(response) {
  const labels={general:"Через Zapret · list-general-user.txt",exclude:"В исключениях · list-exclude-user.txt",none:"Не добавлен в user-листы",both:"Конфликт: домен есть в обоих user-листах"};
  $("state").textContent=labels[response.state] || "Статус неизвестен";
  for(const id of ["general","exclude"])$(id).classList.toggle("selected",response.state===id);
}
async function request(action) {
  if (busy) return;busy=true;controls();$("message").textContent="";
  try {
    const response=await native({action,domain});
    if(!response?.ok)throw new Error(response?.error || "Помощник не вернул результат");
    if(action==="open_gui")$("message").textContent="Программа открыта";
    else {show(response);if(action!=="status")$("message").textContent="Списки обновлены";}
  }catch(error){
    if(action!=="open_gui")$("state").textContent="Состояние не получено";
    $("message").textContent=error.message+"\nОткройте Zapret GUI → Браузеры и проверьте подключение.";
  }finally{busy=false;controls();}
}
for(const id of ["general","exclude"])$(id).addEventListener("click",()=>request(id));
$("refresh").addEventListener("click",()=>request("status"));$("open").addEventListener("click",()=>request("open_gui"));
(async()=>{
  try{
    const [tab]=await tabs();const url=new URL(tab?.url || "");
    if(!["https:","http:"].includes(url.protocol))throw new Error("Откройте обычный HTTP/HTTPS-сайт");
    domain=url.hostname.toLowerCase().replace(/\.$/,"");$("domain").textContent=domain;controls();await request("status");
  }catch(error){$("domain").textContent="Нет домена сайта";$("state").textContent="Недоступно на этой странице";$("message").textContent=error.message;controls();}
})();
