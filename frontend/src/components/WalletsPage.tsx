import { Check, Copy, ExternalLink, Trash2, WalletCards } from 'lucide-react'
import { useState } from 'react'
import { api } from '../api'
import type { WalletMention } from '../types'
import { ago, shortAddress } from '../utils/format'
import { ChainIcon, ChannelAvatar } from './TokenIdentity'
import './WalletsPage.css'

const gmgnChain = (chain: string) => {
  switch (chain.toLowerCase()) {
    case 'ethereum':
    case 'eth': return 'eth'
    case 'solana':
    case 'sol': return 'sol'
    case 'bsc':
    case 'bnb':
    case 'binance': return 'bsc'
    case 'tron':
    case 'trx': return 'tron'
    case 'robinhood': return 'robinhood'
    case 'base': return 'base'
    case 'arbitrum':
    case 'arb': return 'arb'
    case 'optimism':
    case 'op': return 'op'
    case 'polygon':
    case 'matic': return 'polygon'
    default: return null
  }
}

const gmgnWalletUrl = (wallet: WalletMention) => {
  const chain = gmgnChain(wallet.chain_id)
  return chain ? `https://gmgn.ai/${chain}/address/${wallet.address}` : null
}

export function WalletsPage({ wallets, refresh }: { wallets: WalletMention[]; refresh: () => Promise<void> }) {
  const [copiedId, setCopiedId] = useState<number | null>(null)
  const [deleteId, setDeleteId] = useState<number | null>(null)

  const copy = async (wallet: WalletMention) => {
    await navigator.clipboard.writeText(wallet.address)
    setCopiedId(wallet.id)
    window.setTimeout(() => setCopiedId(current => current === wallet.id ? null : current), 1600)
  }

  const remove = async (wallet: WalletMention) => {
    // Use an inline Yes/No confirmation so deleting a wallet never opens a blocking browser-native dialog.
    await api.removeWallet(wallet.id)
    setDeleteId(null)
    await refresh()
  }

  return <main className="page wallets-page"><div className="title"><div><h1>Wallets</h1><p>Verified wallet addresses detected in monitored channels.</p></div></div><div className="wallet-table"><table><thead><tr><th>Wallet</th><th>Chain</th><th>Channel</th><th>Time</th><th>Original message</th><th>Actions</th></tr></thead><tbody>{wallets.map(wallet => { const gmgn = gmgnWalletUrl(wallet); const copied = copiedId === wallet.id; return <tr key={wallet.id}><td><span className="wallet-address"><WalletCards/><b>{shortAddress(wallet.address)}</b><button className={`wallet-copy ${copied ? 'copied' : ''}`} aria-label={copied ? 'Copied to clipboard' : 'Copy wallet'} data-tooltip={copied ? 'Copied to clipboard' : 'Copy wallet'} onClick={() => void copy(wallet)}>{copied ? <Check/> : <Copy/>}</button></span></td><td><span className="wallet-chain"><ChainIcon chain={wallet.chain_id}/><span>{wallet.chain_id}</span></span></td><td><span className="channel-cell"><ChannelAvatar channelId={wallet.channel_id} name={wallet.channel_name} photo={wallet.channel_has_photo} size={28}/>{wallet.channel_name}</span></td><td>{ago(wallet.mentioned_at)}</td><td><span className="wallet-message">{wallet.message}</span></td><td><span className="wallet-actions">{gmgn ? <a className="wallet-gmgn" href={gmgn} target="_blank" rel="noreferrer">Open GMGN<ExternalLink/></a> : <span className="wallet-action-unavailable">—</span>}{deleteId === wallet.id ? <span className="wallet-delete-confirm"><button className="wallet-delete-yes" onClick={() => void remove(wallet)}>Yes</button><button className="wallet-delete-no" onClick={() => setDeleteId(null)}>No</button></span> : <button className="wallet-delete" onClick={() => setDeleteId(wallet.id)} title="Delete wallet mention">Delete<Trash2/></button>}</span></td></tr> })}</tbody></table>{wallets.length===0&&<div className="wallet-empty"><WalletCards/><b>No wallets detected</b><span>Verified wallet addresses will appear here automatically.</span></div>}</div></main>
}
