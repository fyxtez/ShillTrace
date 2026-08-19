import { useState, type MouseEvent } from 'react'
import { Check, Copy, Globe2, Send } from 'lucide-react'
import { api } from '../api'
import type { Shill } from '../types'
import { shortAddress } from '../utils/format'
import './TokenIdentity.css'

export function Avatar({ shill, size = 38 }: { shill: Shill, size?: number }) { return shill.image_url ? <img className="avatar" style={{ width: size, height: size }} src={shill.image_url} /> : <span className="avatar fallback" style={{ width: size, height: size }}>{(shill.symbol ?? '?').slice(0, 2)}</span> }
export function ChannelAvatar({ channelId, name, photo, size = 34 }: { channelId: number, name: string, photo: boolean, size?: number }) { return photo ? <img className="avatar" style={{ width: size, height: size }} src={api.photo(channelId)} /> : <span className="avatar channel-avatar" style={{ width: size, height: size }}>{name.slice(0, 2).toUpperCase()}</span> }
export function CopyButton({ value }: { value: string }) {
    const [copied, setCopied] = useState(false)
    // Immediate visual confirmation removes doubt after copying a long contract;
    // the message clears itself so repeated copies still provide feedback.
    const copy = async (event: MouseEvent) => { event.stopPropagation(); await navigator.clipboard.writeText(value); setCopied(true); window.setTimeout(() => setCopied(false), 1600) }
    return <span className="copy-wrap"><button className="copy" title="Copy contract" onClick={copy}><Copy /></button><span className={`copy-feedback ${copied ? 'visible' : ''}`}>CA copied to clipboard</span></span>
}
export function CopyActionButton({ value }: { value: string }) { const [copied, setCopied] = useState(false); return <button onClick={async () => { await navigator.clipboard.writeText(value); setCopied(true); window.setTimeout(() => setCopied(false), 1600) }}>{copied ? <><Check />CA copied to clipboard</> : <>Copy CA<Copy /></>}</button> }

// Small inline marks keep chain identity recognizable without relying on a
// third-party image host that could break the dashboard later.
export function ChainIcon({ chain }: { chain: string | null }) { const id = (chain ?? 'unknown').toLowerCase(); const icons: Record<string, string> = { ethereum: '/chain-icons/ethereum.png', solana: '/chain-icons/solana.png', bsc: '/chain-icons/bsc.png', base: '/chain-icons/base.png', robinhood: '/chain-icons/robinhood.svg', tron: '/chain-icons/tron.svg' }; const src = icons[id]; return src ? <img className="chain-icon" src={src} alt={`${chain} network`} /> : <span className="chain-icon chain-unknown">?</span> }
export function ChainLabel({ chain }: { chain: string | null }) { return <span className="chain-label"><ChainIcon chain={chain} /><span>{chain ?? 'Unknown chain'}</span></span> }

export function TokenSocials({ shill }: { shill: Shill }) {
    const links = [['Website', shill.website_url, <Globe2 />], ['X / Twitter', shill.twitter_url, <span className="x-social-mark">𝕏</span>], ['Telegram', shill.telegram_url, <Send />]] as const
    // Missing links remain visible to preserve a stable layout, but blur and
    // pointer-events prevent them from behaving like available destinations.
    return <span className="token-socials">{links.map(([label, url, icon]) => url ? <a key={label} href={url} target="_blank" rel="noreferrer" title={label} onClick={event => event.stopPropagation()}>{icon}</a> : <span key={label} className="social-disabled" aria-label={`${label} unavailable`}>{icon}</span>)}</span>
}
