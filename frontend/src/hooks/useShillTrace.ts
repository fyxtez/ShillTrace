import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { api } from '../api'
import type { Channel, HistoryPoint, Page, Shill, WalletMention } from '../types'

export function useShillTrace() {
  const [page,setPage]=useState<Page>('new'), [shills,setShills]=useState<Shill[]>([]), [channels,setChannels]=useState<Channel[]>([]), [wallets,setWallets]=useState<WalletMention[]>([]), [selected,setSelected]=useState<Shill|null>(null), [history,setHistory]=useState<HistoryPoint[]>([]), [search,setSearch]=useState(''), [loading,setLoading]=useState(true), [pollSeconds,setPollSeconds]=useState(15)
  const [sidebarCollapsed,setSidebarCollapsed]=useState(()=>sessionStorage.getItem('shilltrace-sidebar-collapsed')==='true')
  const [walletUnseen,setWalletUnseen]=useState(0)
  const alertAudio=useRef<HTMLAudioElement|null>(null)
  useEffect(()=>sessionStorage.setItem('shilltrace-sidebar-collapsed',String(sidebarCollapsed)),[sidebarCollapsed])
  const refresh=useCallback(async()=>{try{const channelHistory=page==='channels'||page==='ignored';const [ss,cs,ws]=await Promise.all([api.shills(page==='new',undefined,channelHistory),api.channels(),api.wallets()]);setShills(ss);setChannels(cs);setWallets(ws);setSelected(old=>old?ss.find(s=>s.id===old.id)??null:page==='new'?ss[0]??null:null)}finally{setLoading(false)}},[page])
  useEffect(()=>{void refresh()},[refresh])
  useEffect(()=>{const audio=new Audio('/alert.mp3');audio.preload='auto';alertAudio.current=audio;const unlock=()=>{audio.muted=true;void audio.play().then(()=>{audio.pause();audio.currentTime=0;audio.muted=false}).catch(()=>{audio.muted=false})};window.addEventListener('pointerdown',unlock,{once:true});return()=>{window.removeEventListener('pointerdown',unlock);audio.pause();alertAudio.current=null}},[])
  // Wallet calls are first-class signals too. Newly verified wallets and repeat
  // wallet mentions raise their own sidebar badge without affecting New Shills.
  useEffect(()=>{const events=new EventSource(api.events);events.onmessage=event=>{try{const type=JSON.parse(event.data).type;const isWallet=type==='new_wallet'||type==='candidate_reclassified_wallet';if(type==='new_shill'||isWallet){const audio=alertAudio.current;if(audio){audio.currentTime=0;void audio.play().catch(()=>{})}}if(isWallet&&page!=='wallets')setWalletUnseen(count=>count+1)}catch{/* malformed events must not stop refreshes */}void refresh()};return()=>events.close()},[refresh,page])
  useEffect(()=>{const timer=window.setInterval(refresh,15000);return()=>window.clearInterval(timer)},[refresh])
  useEffect(()=>{selected?api.history(selected.id).then(setHistory).catch(()=>setHistory([])):setHistory([])},[selected])
  useEffect(()=>{api.health().then(h=>setPollSeconds(h.market_poll_seconds)).catch(()=>setPollSeconds(15))},[])
  useEffect(()=>{const close=(e:KeyboardEvent)=>{if(e.key==='Escape')setSelected(null)};window.addEventListener('keydown',close);return()=>window.removeEventListener('keydown',close)},[])
  const filtered=useMemo(()=>shills.filter(s=>`${s.symbol} ${s.token_name} ${s.contract_address} ${s.channel_name}`.toLowerCase().includes(search.toLowerCase())),[shills,search])
  const unseen=shills.filter(s=>!s.seen_at).length
  useEffect(()=>{document.title=unseen>0?`(${unseen}) ShillTrace`:'ShillTrace';return()=>{document.title='ShillTrace'}},[unseen])
  const act=async(fn:()=>Promise<unknown>)=>{await fn();await refresh()}
  const changePage=(next:Page)=>{setSelected(null);if(next==='wallets')setWalletUnseen(0);setPage(next)}
  return {page,setPage,changePage,shills,channels,wallets,selected,setSelected,history,search,setSearch,loading,pollSeconds,sidebarCollapsed,setSidebarCollapsed,filtered,unseen,walletUnseen,refresh,act}
}
