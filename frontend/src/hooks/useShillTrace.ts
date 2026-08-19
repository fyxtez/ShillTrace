import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../api'
import type { Channel, HistoryPoint, Page, Shill } from '../types'

export function useShillTrace() {
  const [page,setPage]=useState<Page>('new'), [shills,setShills]=useState<Shill[]>([]), [channels,setChannels]=useState<Channel[]>([]), [selected,setSelected]=useState<Shill|null>(null), [history,setHistory]=useState<HistoryPoint[]>([]), [search,setSearch]=useState(''), [loading,setLoading]=useState(true), [pollSeconds,setPollSeconds]=useState(15)
  const [sidebarCollapsed,setSidebarCollapsed]=useState(()=>sessionStorage.getItem('shilltrace-sidebar-collapsed')==='true')
  const alertAudio=useRef<HTMLAudioElement|null>(null)
  useEffect(()=>sessionStorage.setItem('shilltrace-sidebar-collapsed',String(sidebarCollapsed)),[sidebarCollapsed])
  const refresh=useCallback(async()=>{try{const channelHistory=page==='channels'||page==='ignored';const [ss,cs]=await Promise.all([api.shills(page==='new',undefined,channelHistory),api.channels()]);setShills(ss);setChannels(cs);setSelected(old=>old?ss.find(s=>s.id===old.id)??null:page==='new'?ss[0]??null:null)}finally{setLoading(false)}},[page])
  useEffect(()=>{void refresh()},[refresh])
  useEffect(()=>{const audio=new Audio('/alert.mp3');audio.preload='auto';alertAudio.current=audio;const unlock=()=>{audio.muted=true;void audio.play().then(()=>{audio.pause();audio.currentTime=0;audio.muted=false}).catch(()=>{audio.muted=false})};window.addEventListener('pointerdown',unlock,{once:true});return()=>{window.removeEventListener('pointerdown',unlock);audio.pause();alertAudio.current=null}},[])
  useEffect(()=>{const events=new EventSource(api.events);events.onmessage=event=>{try{if(JSON.parse(event.data).type==='new_shill'){const audio=alertAudio.current;if(audio){audio.currentTime=0;void audio.play().catch(()=>{})}}}catch{/* malformed events must not stop refreshes */}void refresh()};return()=>events.close()},[refresh])
  useEffect(()=>{const timer=window.setInterval(refresh,15000);return()=>window.clearInterval(timer)},[refresh])
  useEffect(()=>{selected?api.history(selected.id).then(setHistory).catch(()=>setHistory([])):setHistory([])},[selected])
  useEffect(()=>{api.health().then(h=>setPollSeconds(h.market_poll_seconds)).catch(()=>setPollSeconds(15))},[])
  useEffect(()=>{const close=(e:KeyboardEvent)=>{if(e.key==='Escape')setSelected(null)};window.addEventListener('keydown',close);return()=>window.removeEventListener('keydown',close)},[])
  const filtered=useMemo(()=>shills.filter(s=>`${s.symbol} ${s.token_name} ${s.contract_address} ${s.channel_name}`.toLowerCase().includes(search.toLowerCase())),[shills,search])
  const unseen=shills.filter(s=>!s.seen_at).length
  useEffect(()=>{document.title=unseen>0?`(${unseen}) ShillTrace`:'ShillTrace';return()=>{document.title='ShillTrace'}},[unseen])
  const act=async(fn:()=>Promise<unknown>)=>{await fn();await refresh()}
  const changePage=(next:Page)=>{setSelected(null);setPage(next)}
  return {page,setPage,changePage,shills,channels,selected,setSelected,history,search,setSearch,loading,pollSeconds,sidebarCollapsed,setSidebarCollapsed,filtered,unseen,refresh,act}
}
