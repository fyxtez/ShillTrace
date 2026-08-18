import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from 'react'
import { BellRing, Check, CircleSlash2, Copy, Database, ExternalLink, Eye, EyeOff, Globe2, Mail, Menu, Pin, PinOff, RefreshCw, Search, Send, Trash2, Users, X } from 'lucide-react'
import { api } from './api'
import type { Channel, HistoryPoint, Page, Shill } from './types'

const cap = (v: number | null) => v == null ? 'Unavailable' : v >= 1e9 ? `$${(v / 1e9).toFixed(2)}B` : v >= 1e6 ? `$${(v / 1e6).toFixed(2)}M` : v >= 1e3 ? `$${(v / 1e3).toFixed(2)}K` : `$${v.toFixed(2)}`
const ratio = (v: number | null, i: number | null) => v != null && i ? v / i : null
const fx = (v: number | null) => v == null ? '—' : `${v.toFixed(2)}x`
const short = (v: string) => `${v.slice(0, 7)}…${v.slice(-5)}`
function ago(iso: string) { const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000); if (s < 60) return `${Math.floor(s)}s ago`; if (s < 3600) return `${Math.floor(s / 60)}m ago`; if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor(s % 3600 / 60)}m ago`; if (s < 172800) return 'Yesterday'; return `${Math.floor(s / 86400)}d ago` }
function longAgo(iso: string) { const days = Math.floor(Math.max(0, Date.now() - new Date(iso).getTime()) / 86_400_000); return days < 1 ? ago(iso) : days === 1 ? '1 day ago' : `${days} days ago` }

function Avatar({ shill, size = 38 }: { shill: Shill, size?: number }) { return shill.image_url ? <img className="avatar" style={{ width: size, height: size }} src={shill.image_url} /> : <span className="avatar fallback" style={{ width: size, height: size }}>{(shill.symbol ?? '?').slice(0, 2)}</span> }
function ChannelAvatar({ channelId, name, photo, size = 34 }: { channelId: number, name: string, photo: boolean, size?: number }) { return photo ? <img className="avatar" style={{ width: size, height: size }} src={api.photo(channelId)} /> : <span className="avatar channel-avatar" style={{ width: size, height: size }}>{name.slice(0, 2).toUpperCase()}</span> }
function CopyButton({ value }: { value: string }) {
    const [copied, setCopied] = useState(false)
    // Immediate visual confirmation removes doubt after copying a long contract;
    // the message clears itself so repeated copies still provide feedback.
    const copy = async (event: MouseEvent) => { event.stopPropagation(); await navigator.clipboard.writeText(value); setCopied(true); window.setTimeout(() => setCopied(false), 1600) }
    return <span className="copy-wrap"><button className="copy" title="Copy contract" onClick={copy}><Copy /></button><span className={`copy-feedback ${copied ? 'visible' : ''}`}>CA copied to clipboard</span></span>
}
function CopyActionButton({ value }: { value: string }) { const [copied, setCopied] = useState(false); return <button onClick={async () => { await navigator.clipboard.writeText(value); setCopied(true); window.setTimeout(() => setCopied(false), 1600) }}>{copied ? <><Check />CA copied to clipboard</> : <>Copy CA<Copy /></>}</button> }

// Small inline marks keep chain identity recognizable without relying on a
// third-party image host that could break the dashboard later.
function ChainIcon({ chain }: { chain: string | null }) { const id = (chain ?? 'unknown').toLowerCase(); const icons: Record<string, string> = { ethereum: '/chain-icons/ethereum.png', solana: '/chain-icons/solana.png', bsc: '/chain-icons/bsc.png', base: '/chain-icons/base.png', robinhood: '/chain-icons/robinhood.svg', tron: '/chain-icons/tron.svg' }; const src = icons[id]; return src ? <img className="chain-icon" src={src} alt={`${chain} network`} /> : <span className="chain-icon chain-unknown">?</span> }
function ChainLabel({ chain }: { chain: string | null }) { return <span className="chain-label"><ChainIcon chain={chain} /><span>{chain ?? 'Unknown chain'}</span></span> }

function TokenSocials({ shill }: { shill: Shill }) {
    const links = [['Website', shill.website_url, <Globe2 />], ['X / Twitter', shill.twitter_url, <span className="x-social-mark">𝕏</span>], ['Telegram', shill.telegram_url, <Send />]] as const
    // Missing links remain visible to preserve a stable layout, but blur and
    // pointer-events prevent them from behaving like available destinations.
    return <span className="token-socials">{links.map(([label, url, icon]) => url ? <a key={label} href={url} target="_blank" rel="noreferrer" title={label} onClick={event => event.stopPropagation()}>{icon}</a> : <span key={label} className="social-disabled" aria-label={`${label} unavailable`}>{icon}</span>)}</span>
}

function Sidebar({ page, setPage, unseen, collapsed, setCollapsed }: { page: Page, setPage: (p: Page) => void, unseen: number, collapsed: boolean, setCollapsed: (value: boolean) => void }) {
    // The hamburger now toggles a compact sidebar instead of presenting a dead control.
    const items: [Page, string, ReactNode][] = [['new', 'New Shills', <BellRing />], ['all', 'All Tokens', <Database />], ['channels', 'Channels', <Users />], ['ignored', 'Ignored Channels', <CircleSlash2 />]]
    // Contact shortcuts stay close to connection status without consuming a full
    // settings page, and collapse together with the compact sidebar.
    return <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`}><div className="brand"><button aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'} title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'} onClick={() => setCollapsed(!collapsed)}><Menu /></button><span>ShillTrace</span></div><nav>{items.map(([id, label, icon]) => <button title={collapsed ? label : undefined} key={id} className={page === id ? 'active' : ''} onClick={() => setPage(id)}>{icon}<span>{label}</span>{id === 'new' && unseen > 0 && <b>{unseen}</b>}</button>)}</nav><div className="sidebar-bottom"><div className="contact-card"><a href="https://t.me/fyxtez" target="_blank" rel="noreferrer"><Send /><span><small>Telegram</small>@fyxtez</span><ExternalLink /></a><a href="mailto:fyxtez@gmail.com"><Mail /><span><small>Email</small>fyxtez@gmail.com</span><ExternalLink /></a></div><div className="connection"><strong><i /><span>Live · Connected</span></strong><small>Updates stream automatically</small></div></div></aside>
}

function Filters({ search, setSearch }: { search: string, setSearch: (v: string) => void }) {
    // Search is the only global token control; hidden chain/channel state used to
    // make fresh shills appear missing even though the unseen badge increased.
    return <div className="filters"><label className="search"><Search /><input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search tokens, channels, contracts…" />{search && <button aria-label="Clear search" onClick={() => setSearch('')}><X /></button>}</label></div>
}

function InboxCard({ shill, selected, onSelect, onSeen }: { shill: Shill, selected: boolean, onSelect: () => void, onSeen: () => void }) {
    const current = ratio(shill.current_market_cap, shill.initial_market_cap)
    const [contractCopied, setContractCopied] = useState(false)
    const dex = shill.chain_id && shill.pair_address ? `https://dexscreener.com/${shill.chain_id}/${shill.pair_address}` : `https://dexscreener.com/search?q=${encodeURIComponent(shill.contract_address)}`
    const terminal = shill.chain_id ? `https://trade.padre.gg/trade/${shill.chain_id}/${shill.contract_address}` : null
    // New-shill cards expose the three frequent actions inline so review does not
    // require opening the detail panel just to copy or inspect a contract.
    // The entire contract strip is the copy target, so users can click the address
    // itself instead of aiming at a small icon at the far edge.
    return <article className={`inbox-card ${selected ? 'selected' : ''}`} onClick={onSelect}><header><ChannelAvatar channelId={shill.channel_id} name={shill.channel_name} photo={shill.channel_has_photo} /><div><b>{shill.channel_name}</b><small>{new Date(shill.shilled_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</small></div></header><button className={`inbox-contract ${contractCopied ? 'copied' : ''}`} title="Copy contract address" onClick={async event => { event.stopPropagation(); await navigator.clipboard.writeText(shill.contract_address); setContractCopied(true); window.setTimeout(() => setContractCopied(false), 1600) }}><span><small>Contract address</small><b>{shill.contract_address}</b></span><strong>{contractCopied ? <Check /> : <Copy />}{contractCopied ? 'Copied' : 'Copy CA'}</strong></button><footer><div className="inbox-token"><Avatar shill={shill} size={30} /><span><b>{shill.symbol ?? 'Unknown'}</b><small><ChainLabel chain={shill.chain_id} /></small></span></div><dl><div><dt>Initial MC</dt><dd>{cap(shill.initial_market_cap)}</dd></div><div><dt>Current X</dt><dd className={(current ?? 0) >= 1 ? 'gain' : 'loss'}>{fx(current)}</dd></div></dl><button className="seen" onClick={e => { e.stopPropagation(); onSeen() }}><Check />Seen</button></footer><div className="inbox-quick"><a href={dex} target="_blank" rel="noreferrer" onClick={e => e.stopPropagation()}>DEX<ExternalLink /></a>{terminal && <a href={terminal} target="_blank" rel="noreferrer" onClick={e => e.stopPropagation()}>Terminal<ExternalLink /></a>}</div></article>
}

function groupLabel(iso: string) { const date = new Date(iso), today = new Date(); date.setHours(0, 0, 0, 0); today.setHours(0, 0, 0, 0); const days = Math.round((today.getTime() - date.getTime()) / 86_400_000), formatted = date.toLocaleDateString([], { day: '2-digit', month: 'short', year: 'numeric' }); return days === 0 ? 'Today' : days === 1 ? `Yesterday · ${formatted}` : `${days} days ago · ${formatted}` }
function TokenTable({ shills, selected, onSelect, groupByDay = false }: { shills: Shill[], selected: Shill | null, onSelect: (s: Shill) => void, groupByDay?: boolean }) {
    // All Tokens is a chronological archive, so day separators make long shill
    // histories scannable without changing the underlying newest-first order.
    let previousGroup = ''
    // The resolved project name owns the flexible space between token identity
    // and social actions, keeping symbol and chain together as one visual unit.
    return <div className="table-box"><table><thead><tr><th>Token</th><th>Channel</th><th>Time Shilled</th><th>Initial MC</th><th>Current MC</th><th>Current X</th><th>Max X</th><th>Status</th></tr></thead><tbody>{shills.map(s => { const current = ratio(s.current_market_cap, s.initial_market_cap), max = ratio(s.max_market_cap, s.initial_market_cap), group = groupLabel(s.shilled_at), showGroup = groupByDay && group !== previousGroup; previousGroup = group; return <Fragment key={s.id}>{showGroup && <tr className="day-divider"><td colSpan={8}><span>{group}</span></td></tr>}<tr className={selected?.id === s.id ? 'selected-row' : ''} onClick={() => onSelect(s)}><td><span className="token-cell"><Avatar shill={s} size={32} /><span className="token-identity"><span className="symbol-line"><b>{s.symbol ?? short(s.contract_address)}</b><CopyButton value={s.contract_address} /></span><ChainLabel chain={s.chain_id} /></span><span className="token-name">{s.token_name ?? 'Name unavailable'}</span><TokenSocials shill={s} /></span></td><td><span className="channel-cell"><ChannelAvatar channelId={s.channel_id} name={s.channel_name} photo={s.channel_has_photo} size={28} />{s.channel_name}</span></td><td>{ago(s.shilled_at)}</td><td>{cap(s.initial_market_cap)}</td><td>{cap(s.current_market_cap)}</td><td className={(current ?? 0) >= 1 ? 'gain' : 'loss'}>{fx(current)}</td><td className="gain">{fx(max)}</td><td><span className={`status ${s.seen_at ? '' : 'new'}`}>{s.seen_at ? 'Seen' : 'New'}</span></td></tr></Fragment> })}</tbody></table>{!shills.length && <div className="empty"><BellRing /><b>No shills found</b><span>New Telegram calls will appear here.</span></div>}</div>
}

function Chart({ points, pollSeconds, shilledAt }: { points: HistoryPoint[], pollSeconds: number, shilledAt: string }) {
    const [hoveredIndex, setHoveredIndex] = useState<number | null>(null)
    // Explicit plot margins leave room for readable market-cap and time axes;
    // the previous full-bleed line had no scale, so its movement was ambiguous.
    const plot = useMemo(() => { if (points.length < 2) return null; const values = points.map(point => point.market_cap), rawMin = Math.min(...values), rawMax = Math.max(...values), padding = Math.max((rawMax - rawMin) * .12, rawMax * .002, 1), min = Math.max(0, rawMin - padding), max = rawMax + padding, range = Math.max(1, max - min), left = 92, right = 985, top = 16, bottom = 215; const x = (index: number) => left + index / (points.length - 1) * (right - left), y = (value: number) => bottom - (value - min) / range * (bottom - top); return { line: points.map((point, index) => `${x(index)},${y(point.market_cap)}`).join(' '), min, max, left, right, top, bottom, x, y } }, [points])
    const first = points[0]?.market_cap ?? null, current = points.at(-1)?.market_cap ?? null
    const yTicks = plot ? Array.from({ length: 4 }, (_, index) => plot.min + (plot.max - plot.min) * (3 - index) / 3) : []
    // Two-point charts previously produced [0, 0, 1], giving React duplicate
    // child keys on every refresh. Set preserves order while removing collisions.
    const xIndexes = points.length > 1 ? [...new Set([0, Math.floor((points.length - 1) / 2), points.length - 1])] : []
    const timeLabel = (iso: string) => new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    const ageSeconds = Math.max(0, (Date.now() - new Date(shilledAt).getTime()) / 1000), sampleCadence = ageSeconds <= 604_800 ? '1 min' : ageSeconds <= 2_592_000 ? '5 min' : '1 hour'
    // SVG title tooltips have a browser-controlled delay. This in-chart tooltip
    // is rendered on pointer entry, so exact values appear immediately.
    const hovered = hoveredIndex == null ? null : points[hoveredIndex]
    const tooltipX = hovered && plot ? Math.min(830, Math.max(100, plot.x(hoveredIndex!) - 135)) : 0
    const tooltipY = hovered && plot ? Math.max(8, plot.y(hovered.market_cap) - 48) : 0
    return <div className="chart"><header><div><b>Market cap over time</b><small>Live MC refresh: every {pollSeconds}s · chart sample: every {sampleCadence} · hover for exact data.</small></div><div className="chart-legend"><span><i className="start" />Initial {cap(first)}</span><span><i />Latest {cap(current)}</span></div></header>{plot ? <svg className="market-chart" viewBox="0 0 1000 260" role="img" aria-label="Market cap history chart" onMouseLeave={() => setHoveredIndex(null)}>{yTicks.map(value => { const y = plot.y(value); return <g key={value}><line className="chart-grid" x1={plot.left} x2={plot.right} y1={y} y2={y} /><text className="axis-label axis-y" x={plot.left - 12} y={y + 4}>{cap(value)}</text></g> })}<line className="chart-axis" x1={plot.left} x2={plot.left} y1={plot.top} y2={plot.bottom} /><line className="chart-axis" x1={plot.left} x2={plot.right} y1={plot.bottom} y2={plot.bottom} /><polyline points={plot.line} />{points.map((point, index) => <circle className={`chart-point ${point.is_initial ? 'initial-point' : ''}`} key={`${point.time}-${index}`} cx={plot.x(index)} cy={plot.y(point.market_cap)} r={point.is_initial ? 5 : 4} onMouseEnter={() => setHoveredIndex(index)} />)}{xIndexes.map(index => <text className="axis-label axis-x" key={index} x={plot.x(index)} y="244" textAnchor={index === 0 ? 'start' : index === points.length - 1 ? 'end' : 'middle'}>{timeLabel(points[index].time)}</text>)}{hovered && <g className="chart-tooltip" transform={`translate(${tooltipX} ${tooltipY})`}><rect width="270" height="34" rx="4" /><text x="12" y="22">{new Date(hovered.time).toLocaleString()} · {cap(hovered.market_cap)}</text></g>}</svg> : <span className="no-chart">Two samples are needed before a trend line can be drawn.</span>}</div>
}

function Detail({ shill, history, pollSeconds, onRemove, onRetry }: { shill: Shill, history: HistoryPoint[], pollSeconds: number, onRemove: () => void, onRetry: () => void }) {
    const [confirm, setConfirm] = useState(false), [contractCopied, setContractCopied] = useState(false), current = ratio(shill.current_market_cap, shill.initial_market_cap), max = ratio(shill.max_market_cap, shill.initial_market_cap)
    const dex = shill.chain_id && shill.pair_address ? `https://dexscreener.com/${shill.chain_id}/${shill.pair_address}` : `https://dexscreener.com/search?q=${encodeURIComponent(shill.contract_address)}`
    const padre = shill.chain_id ? `https://trade.padre.gg/trade/${shill.chain_id}/${shill.contract_address}` : null
    // Contract identity belongs in the hero beside the symbol; the metadata row
    // can then balance source channel against the resolved project name.
    // Current X represents profit only from 1x upward; applying the same threshold
    // used by cards and tables makes sub-1x drawdowns red in the detail hero.
    // A dedicated backdrop keeps destructive confirmation above the sticky
    // action footer; nesting the old popover in the detail stacking context let
    // the full-width Remove button paint over its text and controls.
    return <section className="detail"><header className="detail-hero"><Avatar shill={shill} size={62} /><div className="detail-identity"><div className="detail-title-line"><h2>{shill.symbol ?? 'Unresolved token'}</h2><button className={`hero-contract ${contractCopied ? 'copied' : ''}`} title="Copy contract address" onClick={async () => { await navigator.clipboard.writeText(shill.contract_address); setContractCopied(true); window.setTimeout(() => setContractCopied(false), 1600) }}><span>{short(shill.contract_address)}</span>{contractCopied ? <Check /> : <Copy />}</button></div><ChainLabel chain={shill.chain_id} /></div><div className="hero-token-name"><small>Token name</small><b>{shill.token_name ?? 'Name unavailable'}</b></div><TokenSocials shill={shill} /><div className="big-x"><b className={(current ?? 0) >= 1 ? 'gain' : 'loss'}>{fx(current)}</b><small>Current X</small></div></header><div className="metrics"><Metric label="Initial MC" value={cap(shill.initial_market_cap)} /><Metric label="Current MC" value={cap(shill.current_market_cap)} /><Metric label="Max X" value={fx(max)} positive /><Metric label="Time shilled" value={ago(shill.shilled_at)} /></div>{shill.market_status === 'unavailable' && <div className="warning"><div><b>Initial market cap unavailable</b><span>DEX Screener may have current data while historical candles are unavailable. Retry to resolve it now.</span></div><button onClick={onRetry}><RefreshCw />Retry</button></div>}<div className="detail-content"><div><div className="meta"><span><small>Channel</small><b><ChannelAvatar channelId={shill.channel_id} name={shill.channel_name} photo={shill.channel_has_photo} size={24} />{shill.channel_name}</b></span></div><div className="message"><small>Original message</small><b><ChannelAvatar channelId={shill.channel_id} name={shill.channel_name} photo={shill.channel_has_photo} size={34} />{shill.channel_name}</b><p>{shill.message}</p></div></div><Chart points={history} pollSeconds={pollSeconds} shilledAt={shill.shilled_at} /></div><footer className="detail-actions"><a href={dex} target="_blank">Open DEX chart<ExternalLink /></a>{padre && <a href={padre} target="_blank">Open Terminal chart<ExternalLink /></a>}<CopyActionButton value={shill.contract_address} /><button className="remove" onClick={() => setConfirm(true)}>Remove token<Trash2 /></button></footer>{confirm && <div className="confirm-backdrop" onMouseDown={() => setConfirm(false)}><div className="confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby="remove-token-title" onMouseDown={event => event.stopPropagation()}><b id="remove-token-title">Permanently remove {shill.symbol ?? 'this token'}?</b><span>This deletes the token, its shills, tracking periods, and every market-cap sample. This cannot be undone.</span><div className="confirm-actions"><button onClick={() => setConfirm(false)}>Cancel</button><button className="danger" onClick={onRemove}>Remove permanently</button></div></div></div>}</section>
}
function Metric({ label, value, positive }: { label: string, value: string, positive?: boolean }) { return <div className="metric"><small>{label}</small><b className={positive ? 'gain' : ''}>{value}</b></div> }

function ChannelsPage({ channels, shills, ignored, refresh, onOpenShill }: { channels: Channel[], shills: Shill[], ignored: boolean, refresh: () => void, onOpenShill: (shill: Shill) => void }) {
    // Channel search is local and immediate because the complete channel list is
    // already loaded; no extra backend request is needed for every keystroke.
    const [channelSearch, setChannelSearch] = useState('')
    const [showHidden, setShowHidden] = useState(false)
    // Hidden ignored channels remain persisted and searchable on demand; the
    // toggle only changes presentation and never resumes Telegram monitoring.
    const visibleChannels = useMemo(() => channels.filter(channel => channel.is_ignored === ignored && (!ignored || showHidden || !channel.is_hidden) && channel.name.toLowerCase().includes(channelSearch.trim().toLowerCase())), [channels, ignored, showHidden, channelSearch])
    const hiddenCount = channels.filter(channel => channel.is_ignored && channel.is_hidden).length
    const pinned = visibleChannels.filter(channel => channel.is_pinned), all = visibleChannels.filter(channel => !channel.is_pinned)
    const lastShill = (channel: Channel) => channel.last_shill_at ? `Last shill: ${longAgo(channel.last_shill_at)} (${new Date(channel.last_shill_at).toLocaleDateString([], { day: '2-digit', month: 'short', year: 'numeric' })})` : 'Last shill: never'
    // Pinned and regular monitored channels are separate, non-duplicated groups;
    // ignored channels retain one list and emphasize their most recent activity.
    // A shill row is a navigation shortcut into the canonical All Tokens detail
    // view, preserving one modal implementation instead of duplicating details.
    const renderChannel = (c: Channel) => { const channelShills = shills.filter(shill => shill.channel_id === c.telegram_id); return <article className="channel-column" key={c.telegram_id}><header><ChannelAvatar channelId={c.telegram_id} name={c.name} photo={c.has_photo} size={56} /><div><b>{c.name}</b><small>{c.shill_count} shills · Median {fx(c.median_current_x)}</small>{ignored && <small className="last-shill">{lastShill(c)}</small>}</div><span className="channel-actions">{!ignored && <button className={`pin-channel ${c.is_pinned ? 'active' : ''}`} title={c.is_pinned ? 'Unpin channel' : 'Pin channel'} onClick={async () => { await api.setPinned(c.telegram_id, !c.is_pinned); refresh() }}>{c.is_pinned ? <PinOff /> : <Pin />}{c.is_pinned ? 'Unpin' : 'Pin'}</button>}{ignored && <button className="hide-channel" onClick={async () => { await api.setHidden(c.telegram_id, !c.is_hidden); refresh() }}>{c.is_hidden ? <Eye /> : <EyeOff />}{c.is_hidden ? 'Show channel' : 'Hide channel'}</button>}<button onClick={async () => { await api.setIgnored(c.telegram_id, !c.is_ignored); refresh() }}>{c.is_ignored ? 'Start monitoring' : 'Ignore channel'}</button></span></header><section className="channel-shills">{channelShills.map(shill => { const current = ratio(shill.current_market_cap, shill.initial_market_cap); return <div className="channel-shill" role="button" tabIndex={0} key={shill.id} onClick={() => onOpenShill(shill)} onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onOpenShill(shill) } }}><Avatar shill={shill} size={40} /><div><b>{shill.symbol ?? short(shill.contract_address)}</b><small><ChainIcon chain={shill.chain_id} />{ago(shill.shilled_at)}</small></div><dl><span><dt>Initial</dt><dd>{cap(shill.initial_market_cap)}</dd></span><span><dt>Current</dt><dd>{cap(shill.current_market_cap)}</dd></span><span><dt>X</dt><dd className={(current ?? 0) >= 1 ? 'gain' : 'loss'}>{fx(current)}</dd></span></dl></div> })}{!channelShills.length && <div className="no-channel-shills">No shills recorded yet</div>}</section></article> }
    const section = (title: string, items: Channel[]) => <section className="channel-section"><h2>{title}<b>{items.length}</b></h2>{items.length ? <div className="channel-grid channel-columns">{items.map(renderChannel)}</div> : <div className="channel-section-empty">{title === 'Pinned Channels' ? 'Pin important channels to keep them here.' : 'No matching channels in this section.'}</div>}</section>
    return <main className="page channels-page"><div className="title channel-title"><div><h1>{ignored ? 'Ignored Channels' : 'Channels'}</h1><p>{ignored ? 'Telegram messages from these channels are skipped. Hidden only controls this list.' : 'Monitored Telegram channels and their shill history.'}</p></div><div className="channel-title-actions">{ignored && <button className={`hidden-toggle ${showHidden ? 'active' : ''}`} onClick={() => setShowHidden(value => !value)}>{showHidden ? <EyeOff /> : <Eye />}{showHidden ? 'Hide hidden channels' : `Show hidden channels (${hiddenCount})`}</button>}<label className="search channel-search"><Search /><input value={channelSearch} onChange={event => setChannelSearch(event.target.value)} placeholder={`Search ${ignored ? 'ignored channels' : 'channels'}…`} />{channelSearch && <button aria-label="Clear search" onClick={() => setChannelSearch('')}><X /></button>}</label></div></div><div className="channel-count">Showing <b>{visibleChannels.length}</b> of {channels.filter(channel => channel.is_ignored === ignored).length}</div>{ignored ? section(showHidden ? 'Ignored Channels · including hidden' : 'Ignored Channels', visibleChannels) : <>{section('Pinned Channels', pinned)}{section('All Channels', all)}</>}{!visibleChannels.length && <div className="empty channel-empty"><Search /><b>No matching channels</b><span>{ignored && hiddenCount && !showHidden ? 'Use “Show hidden channels” to include hidden results.' : 'Try a different channel name.'}</span></div>}</main>
}

export default function App() {
    const [page, setPage] = useState<Page>('new'), [shills, setShills] = useState<Shill[]>([]), [channels, setChannels] = useState<Channel[]>([]), [selected, setSelected] = useState<Shill | null>(null), [history, setHistory] = useState<HistoryPoint[]>([]), [search, setSearch] = useState(''), [chain, setChain] = useState('All'), [channelId, setChannelId] = useState<number | null>(null), [loading, setLoading] = useState(true), [pollSeconds, setPollSeconds] = useState(15)
    // Preserve the user's navigation density preference during this browser session.
    // The renamed storage key prevents old Signal Ledger branding from leaking
    // into browser state while keeping this preference local to ShillTrace.
    const [sidebarCollapsed, setSidebarCollapsed] = useState(() => sessionStorage.getItem('shilltrace-sidebar-collapsed') === 'true')
    const alertAudio = useRef<HTMLAudioElement | null>(null)
    useEffect(() => sessionStorage.setItem('shilltrace-sidebar-collapsed', String(sidebarCollapsed)), [sidebarCollapsed])
    const refresh = useCallback(async () => { try { const channelHistory = page === 'channels' || page === 'ignored'; const [ss, cs] = await Promise.all([api.shills(page === 'new', undefined, channelHistory), api.channels()]); setShills(ss); setChannels(cs); setSelected(old => old ? ss.find(s => s.id === old.id) ?? null : page === 'new' ? ss[0] ?? null : null) } finally { setLoading(false) } }, [page])
    useEffect(() => { refresh() }, [refresh]);
    useEffect(() => {
        const audio = new Audio('/alert.mp3'); audio.preload = 'auto'; alertAudio.current = audio
        // Browsers require one user gesture before scripted audio; silently priming
        // the bundled alert on the first gesture lets later SSE shills ring instantly.
        const unlock = () => { audio.muted = true; void audio.play().then(() => { audio.pause(); audio.currentTime = 0; audio.muted = false }).catch(() => { audio.muted = false }) }
        window.addEventListener('pointerdown', unlock, { once: true })
        return () => { window.removeEventListener('pointerdown', unlock); audio.pause(); alertAudio.current = null }
    }, [])
    useEffect(() => {
        const events = new EventSource(api.events)
        events.onmessage = event => {
            // Only a genuinely ingested shill deserves an audible alert; market ticks,
            // seen changes, removals, and SSE reconnects continue refreshing silently.
            try { if (JSON.parse(event.data).type === 'new_shill') { const audio = alertAudio.current; if (audio) { audio.currentTime = 0; void audio.play().catch(() => { }) } } } catch {/* A malformed event must not stop live refreshes. */ }
            void refresh()
        }
        return () => events.close()
    }, [refresh]);
    // Polling is a safety net for browser/SSE interruptions, guaranteeing that a
    // newly ingested shill appears without requiring a manual page refresh.
    useEffect(() => { const timer = window.setInterval(refresh, 15_000); return () => window.clearInterval(timer) }, [refresh]); useEffect(() => { selected ? api.history(selected.id).then(setHistory).catch(() => setHistory([])) : setHistory([]) }, [selected])
    // Health metadata keeps the displayed live cadence synchronized with the
    // backend's MARKET_POLL_SECONDS configuration.
    useEffect(() => { api.health().then(health => setPollSeconds(health.market_poll_seconds)).catch(() => setPollSeconds(15)) }, [])
    // Drawer dismissal follows normal desktop behavior: Escape or clicking the
    // dimmed area closes it, removing the need for another crowded top-right X.
    useEffect(() => { const close = (event: KeyboardEvent) => { if (event.key === 'Escape') setSelected(null) }; window.addEventListener('keydown', close); return () => window.removeEventListener('keydown', close) }, [])
    const filtered = useMemo(() => shills.filter(s => `${s.symbol} ${s.token_name} ${s.contract_address} ${s.channel_name}`.toLowerCase().includes(search.toLowerCase())), [shills, search]); const unseen = shills.filter(s => !s.seen_at).length
    // Mirror the existing unseen badge in the browser tab so missed audio alerts
    // remain visible while the user is working in another tab. Seen actions and
    // live refreshes update the count automatically through React state.
    useEffect(() => { document.title = unseen > 0 ? `(${unseen}) ShillTrace` : 'ShillTrace'; return () => { document.title = 'ShillTrace' } }, [unseen])
    const act = async (fn: () => Promise<unknown>) => { await fn(); await refresh() }
    // Navigating clears the previous selection so All Tokens does not open a
    // drawer until the user explicitly chooses a row.
    const changePage = (next: Page) => { setSelected(null); setPage(next) }
    const sidebarProps = { page, setPage: changePage, unseen, collapsed: sidebarCollapsed, setCollapsed: setSidebarCollapsed }
    // Opening a channel shill switches to All Tokens while retaining that exact
    // row as the selected modal target through the page refresh.
    if (page === 'channels' || page === 'ignored') return <div className={`app ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}><Sidebar {...sidebarProps} /><ChannelsPage channels={channels} shills={shills} ignored={page === 'ignored'} refresh={refresh} onOpenShill={shill => { setSelected(shill); setPage('all') }} /></div>
    return <div className={`app ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}><Sidebar {...sidebarProps} /><main className="page"><div className="title"><div><h1>{page === 'new' ? 'New Shills' : 'All Tokens'}</h1><p>{page === 'new' ? 'Seen moves a shill to All Tokens.' : 'Every detected shill still being tracked.'}</p></div><Filters {...{ search, setSearch, chain, setChain, channels, channelId, setChannelId }} /></div>{loading ? <div className="loading">Loading ShillTrace…</div> : page === 'new' ? <div className="inbox-layout"><section className="review"><h2>Needs review <b>{filtered.length}</b></h2><div className="inbox-list">{filtered.map(s => <InboxCard key={s.id} shill={s} selected={selected?.id === s.id} onSelect={() => setSelected(s)} onSeen={() => act(() => api.seen(s.id))} />)}</div></section><section className="workspace"><TokenTable shills={filtered.slice(0, 7)} selected={selected} onSelect={setSelected} />{selected && <Detail shill={selected} history={history} pollSeconds={pollSeconds} onRetry={() => act(() => api.retryToken(selected.token_id))} onRemove={() => act(() => api.removeToken(selected.token_id))} />}</section></div> : <div className="all-layout"><div className="summary"><Metric label="Tracking" value={`${new Set(filtered.map(s => s.token_id)).size} tokens`} /><Metric label="Channels" value={`${new Set(filtered.map(s => s.channel_id)).size}`} /><Metric label="Median Current" value={fx(median(filtered.map(s => ratio(s.current_market_cap, s.initial_market_cap)).filter((v): v is number => v != null)))} positive /><Metric label="Best Ever" value={fx(Math.max(0, ...filtered.map(s => ratio(s.max_market_cap, s.initial_market_cap) ?? 0)))} positive /></div><TokenTable shills={filtered} selected={selected} onSelect={setSelected} groupByDay />{selected && <div className="drawer-backdrop" onMouseDown={() => setSelected(null)}><div className="drawer" onMouseDown={event => event.stopPropagation()}><Detail shill={selected} history={history} pollSeconds={pollSeconds} onRetry={() => act(() => api.retryToken(selected.token_id))} onRemove={() => act(() => api.removeToken(selected.token_id))} /></div></div>}</div>}</main></div>
}
function median(values: number[]) { if (!values.length) return null; const s = [...values].sort((a, b) => a - b), m = Math.floor(s.length / 2); return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2 }
