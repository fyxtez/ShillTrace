export interface Shill { id:number; token_id:number; symbol:string|null; token_name:string|null; contract_address:string; chain_id:string|null; image_url:string|null; pair_address:string|null; website_url:string|null; twitter_url:string|null; telegram_url:string|null; channel_id:number; channel_name:string; channel_has_photo:boolean; message:string; shilled_at:string; initial_market_cap:number|null; current_market_cap:number|null; max_market_cap:number|null; market_status:string; seen_at:string|null; shill_count:number }
export interface Channel { telegram_id:number; name:string; kind:string; is_ignored:boolean; is_pinned:boolean; has_photo:boolean; shill_count:number; last_shill_at:string|null; average_current_x:number|null; median_current_x:number|null; average_max_x:number|null }
export interface HistoryPoint { time:string; market_cap:number; is_initial?:boolean }
export interface Health { status:string; market_poll_seconds:number }
// Settings is intentionally omitted until the page contains user-controlled
// options; this keeps the primary navigation focused on working features.
export type Page = 'new'|'all'|'channels'|'ignored'
