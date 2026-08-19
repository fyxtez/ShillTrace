export const cap = (v: number | null) => v == null ? 'Unavailable' : v >= 1e9 ? `$${(v / 1e9).toFixed(2)}B` : v >= 1e6 ? `$${(v / 1e6).toFixed(2)}M` : v >= 1e3 ? `$${(v / 1e3).toFixed(2)}K` : `$${v.toFixed(2)}`
export const ratio = (v: number | null, i: number | null) => v != null && i ? v / i : null
export const fx = (v: number | null) => v == null ? '—' : `${v.toFixed(2)}x`
export const shortAddress = (v: string) => `${v.slice(0, 7)}…${v.slice(-5)}`
export function ago(iso: string) { const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000); if (s < 60) return `${Math.floor(s)}s ago`; if (s < 3600) return `${Math.floor(s / 60)}m ago`; if (s < 86400) return `${Math.floor(s / 3600)}h ${Math.floor(s % 3600 / 60)}m ago`; if (s < 172800) return 'Yesterday'; return `${Math.floor(s / 86400)}d ago` }
export function longAgo(iso: string) { const days = Math.floor(Math.max(0, Date.now() - new Date(iso).getTime()) / 86_400_000); return days < 1 ? ago(iso) : days === 1 ? '1 day ago' : `${days} days ago` }
export function median(values: number[]) { if (!values.length) return null; const s = [...values].sort((a,b)=>a-b), m=Math.floor(s.length/2); return s.length%2?s[m]:(s[m-1]+s[m])/2 }
