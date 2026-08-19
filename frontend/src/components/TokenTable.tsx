import { Fragment } from 'react'
import { BellRing } from 'lucide-react'
import type { Shill } from '../types'
import { ago, cap, fx, ratio, shortAddress } from '../utils/format'
import { Avatar, ChainLabel, ChannelAvatar, CopyButton, TokenSocials } from './TokenIdentity'
import './TokenTable.css'

function groupLabel(iso: string) { const date = new Date(iso), today = new Date(); date.setHours(0, 0, 0, 0); today.setHours(0, 0, 0, 0); const days = Math.round((today.getTime() - date.getTime()) / 86_400_000), formatted = date.toLocaleDateString([], { day: '2-digit', month: 'short', year: 'numeric' }); return days === 0 ? 'Today' : days === 1 ? `Yesterday · ${formatted}` : `${days} days ago · ${formatted}` }
export function TokenTable({ shills, selected, onSelect, groupByDay = false }: { shills: Shill[], selected: Shill | null, onSelect: (s: Shill) => void, groupByDay?: boolean }) {
    // All Tokens is a chronological archive, so day separators make long shill
    // histories scannable without changing the underlying newest-first order.
    let previousGroup = ''
    // The resolved project name owns the flexible space between token identity
    // and social actions, keeping symbol and chain together as one visual unit.
    return <div className="table-box"><table><thead><tr><th>Token</th><th>Channel</th><th>Time Shilled</th><th>Initial MC</th><th>Current MC</th><th>Current X</th><th>Max X</th><th>Status</th></tr></thead><tbody>{shills.map(s => { const current = ratio(s.current_market_cap, s.initial_market_cap), max = ratio(s.max_market_cap, s.initial_market_cap), group = groupLabel(s.shilled_at), showGroup = groupByDay && group !== previousGroup; previousGroup = group; return <Fragment key={s.id}>{showGroup && <tr className="day-divider"><td colSpan={8}><span>{group}</span></td></tr>}<tr className={selected?.id === s.id ? 'selected-row' : ''} onClick={() => onSelect(s)}><td><span className="token-cell"><Avatar shill={s} size={32} /><span className="token-identity"><span className="symbol-line"><b>{s.symbol ?? shortAddress(s.contract_address)}</b><CopyButton value={s.contract_address} /></span><ChainLabel chain={s.chain_id} /></span><span className="token-name">{s.token_name ?? 'Name unavailable'}</span><TokenSocials shill={s} /></span></td><td><span className="channel-cell"><ChannelAvatar channelId={s.channel_id} name={s.channel_name} photo={s.channel_has_photo} size={28} />{s.channel_name}</span></td><td>{ago(s.shilled_at)}</td><td>{cap(s.initial_market_cap)}</td><td>{cap(s.current_market_cap)}</td><td className={(current ?? 0) >= 1 ? 'gain' : 'loss'}>{fx(current)}</td><td className="gain">{fx(max)}</td><td><span className={`status ${s.seen_at ? '' : 'new'}`}>{s.seen_at ? 'Seen' : 'New'}</span></td></tr></Fragment> })}</tbody></table>{!shills.length && <div className="empty"><BellRing /><b>No shills found</b><span>New Telegram calls will appear here.</span></div>}</div>
}
