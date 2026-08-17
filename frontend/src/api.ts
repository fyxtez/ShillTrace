import type { Channel, Health, HistoryPoint, Shill } from './types'
const API = import.meta.env.VITE_API_URL ?? 'http://localhost:3001'
async function request<T>(path:string, init?:RequestInit):Promise<T>{const response=await fetch(`${API}${path}`,{...init,headers:{'Content-Type':'application/json',...init?.headers}});if(!response.ok){const body=await response.json().catch(()=>({error:response.statusText}));throw new Error(body.error??'Request failed')}return response.json()}
export const api={
  // Channel pages request removed periods too because they display the full
  // historical record, while token pages remain focused on active tracking.
  shills:(unseen=false,channelId?:number,includeRemoved=false)=>request<Shill[]>(`/api/shills?unseen=${unseen}${channelId?`&channel_id=${channelId}`:''}&include_removed=${includeRemoved}`),
  channels:()=>request<Channel[]>('/api/channels'), seen:(id:number)=>request(`/api/shills/${id}/seen`,{method:'POST'}),
  removeToken:(id:number)=>request(`/api/tokens/${id}`,{method:'DELETE'}), retryToken:(id:number)=>request(`/api/tokens/${id}/retry`,{method:'POST'}),
  setIgnored:(id:number,ignored:boolean)=>request(`/api/channels/${id}/ignored`,{method:'PATCH',body:JSON.stringify({ignored})}),
  setPinned:(id:number,pinned:boolean)=>request(`/api/channels/${id}/pinned`,{method:'PATCH',body:JSON.stringify({pinned})}),
  health:()=>request<Health>('/api/health'),
  history:(shillId:number)=>request<HistoryPoint[]>(`/api/shills/${shillId}/history`), photo:(id:number)=>`${API}/photos/${id}.jpg`, events:`${API}/api/events`,
}
