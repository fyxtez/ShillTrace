import { api } from './api'
import { ChannelsPage } from './components/ChannelsPage'
import { Filters } from './components/Filters'
import { InboxCard } from './components/InboxCard'
import { Sidebar } from './components/Sidebar'
import { Detail, Metric } from './components/TokenDetail'
import { TokenTable } from './components/TokenTable'
import { WalletsPage } from './components/WalletsPage'
import { useShillTrace } from './hooks/useShillTrace'
import { fx, median, ratio } from './utils/format'
import './App.css'

export default function App() {
  const {page,setPage,changePage,shills,channels,wallets,selected,setSelected,history,search,setSearch,loading,pollSeconds,sidebarCollapsed,setSidebarCollapsed,filtered,unseen,walletUnseen,refresh,act}=useShillTrace()
  const sidebarProps={page,setPage:changePage,unseen,walletUnseen,collapsed:sidebarCollapsed,setCollapsed:setSidebarCollapsed}
  if(page==='wallets') return <div className={`app ${sidebarCollapsed?'sidebar-collapsed':''}`}><Sidebar {...sidebarProps}/><WalletsPage wallets={wallets} refresh={refresh}/></div>
  if(page==='channels'||page==='ignored') return <div className={`app ${sidebarCollapsed?'sidebar-collapsed':''}`}><Sidebar {...sidebarProps}/><ChannelsPage channels={channels} shills={shills} ignored={page==='ignored'} refresh={refresh} onOpenShill={shill=>{setSelected(shill);setPage('all')}}/></div>
  return <div className={`app ${sidebarCollapsed?'sidebar-collapsed':''}`}><Sidebar {...sidebarProps}/><main className="page"><div className="title"><div><h1>{page==='new'?'New Shills':'All Tokens'}</h1><p>{page==='new'?'Seen moves a shill to All Tokens.':'Every detected shill still being tracked.'}</p></div><Filters search={search} setSearch={setSearch}/></div>{loading?<div className="loading">Loading ShillTrace…</div>:page==='new'?<div className="inbox-layout"><section className="review"><h2>Needs review <b>{filtered.length}</b></h2><div className="inbox-list">{filtered.map(s=><InboxCard key={s.id} shill={s} selected={selected?.id===s.id} onSelect={()=>setSelected(s)} onSeen={()=>act(()=>api.seen(s.id))}/>)}</div></section><section className="workspace"><TokenTable shills={filtered.slice(0,7)} selected={selected} onSelect={setSelected}/>{selected&&<Detail shill={selected} history={history} pollSeconds={pollSeconds} onRetry={()=>act(()=>api.retryToken(selected.token_id))} onRemove={()=>act(()=>api.removeToken(selected.token_id))}/>}</section></div>:<div className="all-layout"><div className="summary"><Metric label="Tracking" value={`${new Set(filtered.map(s=>s.token_id)).size} tokens`}/><Metric label="Channels" value={`${new Set(filtered.map(s=>s.channel_id)).size}`}/><Metric label="Median Current" value={fx(median(filtered.map(s=>ratio(s.current_market_cap,s.initial_market_cap)).filter((v):v is number=>v!=null)))} positive/><Metric label="Best Ever" value={fx(Math.max(0,...filtered.map(s=>ratio(s.max_market_cap,s.initial_market_cap)??0)))} positive/></div><TokenTable shills={filtered} selected={selected} onSelect={setSelected} groupByDay/>{selected&&<div className="drawer-backdrop" onMouseDown={()=>setSelected(null)}><div className="drawer" onMouseDown={e=>e.stopPropagation()}><Detail shill={selected} history={history} pollSeconds={pollSeconds} onRetry={()=>act(()=>api.retryToken(selected.token_id))} onRemove={()=>act(()=>api.removeToken(selected.token_id))}/></div></div>}</div>}</main></div>
}
