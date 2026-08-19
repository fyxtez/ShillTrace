import { useState } from 'react'
import { Check, Copy, ExternalLink } from 'lucide-react'
import type { Shill } from '../types'
import { cap, fx, ratio } from '../utils/format'
import { Avatar, ChainLabel, ChannelAvatar } from './TokenIdentity'
import './InboxCard.css'

export function InboxCard({ shill, selected, onSelect, onSeen }: { shill: Shill, selected: boolean, onSelect: () => void, onSeen: () => void }) {
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
