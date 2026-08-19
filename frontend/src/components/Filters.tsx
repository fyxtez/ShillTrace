import { Search, X } from 'lucide-react'
import './Filters.css'

export function Filters({ search, setSearch }: { search: string, setSearch: (v: string) => void }) {
    // Search is the only global token control; hidden chain/channel state used to
    // make fresh shills appear missing even though the unseen badge increased.
    return <div className="filters"><label className="search"><Search /><input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search tokens, channels, contracts…" />{search && <button aria-label="Clear search" onClick={() => setSearch('')}><X /></button>}</label></div>
}
