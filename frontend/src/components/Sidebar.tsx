import type { ReactNode } from 'react'
import { BellRing, CircleSlash2, Database, ExternalLink, Mail, Menu, Send, Users } from 'lucide-react'
import type { Page } from '../types'
import './Sidebar.css'

export function Sidebar({ page, setPage, unseen, collapsed, setCollapsed }: { page: Page, setPage: (p: Page) => void, unseen: number, collapsed: boolean, setCollapsed: (value: boolean) => void }) {
    // The hamburger now toggles a compact sidebar instead of presenting a dead control.
    const items: [Page, string, ReactNode][] = [['new', 'New Shills', <BellRing />], ['all', 'All Tokens', <Database />], ['channels', 'Channels', <Users />], ['ignored', 'Ignored Channels', <CircleSlash2 />]]
    // Contact shortcuts stay close to connection status without consuming a full
    // settings page, and collapse together with the compact sidebar.
    return <aside className={`sidebar ${collapsed ? 'collapsed' : ''}`}><div className="brand"><button aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'} title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'} onClick={() => setCollapsed(!collapsed)}><Menu /></button><span>ShillTrace</span></div><nav>{items.map(([id, label, icon]) => <button title={collapsed ? label : undefined} key={id} className={page === id ? 'active' : ''} onClick={() => setPage(id)}>{icon}<span>{label}</span>{id === 'new' && unseen > 0 && <b>{unseen}</b>}</button>)}</nav><div className="sidebar-bottom"><div className="contact-card"><a href="https://t.me/fyxtez" target="_blank" rel="noreferrer"><Send /><span><small>Telegram</small>@fyxtez</span><ExternalLink /></a><a href="mailto:fyxtez@gmail.com"><Mail /><span><small>Email</small>fyxtez@gmail.com</span><ExternalLink /></a></div><div className="connection"><strong><i /><span>Live · Connected</span></strong><small>Updates stream automatically</small></div></div></aside>
}
