import { useMemo, useState, type MouseEvent } from 'react'
import type { HistoryPoint } from '../types'
import { cap } from '../utils/format'
import './MarketChart.css'

export function MarketChart({ points, pollSeconds, shilledAt }: { points: HistoryPoint[], pollSeconds: number, shilledAt: string }) {
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
    // Convert browser pixels into the SVG viewBox and select the nearest sample
    // by time. The whole plot is now interactive, so users no longer have to hit
    // a tiny data-point circle precisely to inspect its timestamp and market cap.
    const inspectAtPointer = (event: MouseEvent<SVGSVGElement>) => {
        if (!plot) return
        const bounds = event.currentTarget.getBoundingClientRect()
        const svgX = (event.clientX - bounds.left) / bounds.width * 1000
        const svgY = (event.clientY - bounds.top) / bounds.height * 260
        if (svgX < plot.left || svgX > plot.right || svgY < plot.top || svgY > plot.bottom) { setHoveredIndex(null); return }
        const ratio = (svgX - plot.left) / (plot.right - plot.left)
        setHoveredIndex(Math.max(0, Math.min(points.length - 1, Math.round(ratio * (points.length - 1)))))
    }
    return <div className="chart"><header><div><b>Market cap over time</b><small>Live MC refresh: every {pollSeconds}s · chart sample: every {sampleCadence} · move across chart for exact data.</small></div><div className="chart-legend"><span><i className="start" />Initial {cap(first)}</span><span><i />Latest {cap(current)}</span></div></header>{plot ? <svg className="market-chart" viewBox="0 0 1000 260" role="img" aria-label="Market cap history chart" onMouseMove={inspectAtPointer} onMouseLeave={() => setHoveredIndex(null)}>{yTicks.map(value => { const y = plot.y(value); return <g key={value}><line className="chart-grid" x1={plot.left} x2={plot.right} y1={y} y2={y} /><text className="axis-label axis-y" x={plot.left - 12} y={y + 4}>{cap(value)}</text></g> })}<line className="chart-axis" x1={plot.left} x2={plot.left} y1={plot.top} y2={plot.bottom} /><line className="chart-axis" x1={plot.left} x2={plot.right} y1={plot.bottom} y2={plot.bottom} /><polyline points={plot.line} />{points.map((point, index) => <circle className={`chart-point ${point.is_initial ? 'initial-point' : ''}`} key={`${point.time}-${index}`} cx={plot.x(index)} cy={plot.y(point.market_cap)} r={point.is_initial ? 5 : 4} />)}{xIndexes.map(index => <text className="axis-label axis-x" key={index} x={plot.x(index)} y="244" textAnchor={index === 0 ? 'start' : index === points.length - 1 ? 'end' : 'middle'}>{timeLabel(points[index].time)}</text>)}{hovered && <><line className="chart-hover-guide" x1={plot.x(hoveredIndex!)} x2={plot.x(hoveredIndex!)} y1={plot.top} y2={plot.bottom} /><circle className="chart-hover-point" cx={plot.x(hoveredIndex!)} cy={plot.y(hovered.market_cap)} r="6" /><g className="chart-tooltip" transform={`translate(${tooltipX} ${tooltipY})`}><rect width="270" height="34" rx="4" /><text x="12" y="22">{new Date(hovered.time).toLocaleString()} · {cap(hovered.market_cap)}</text></g></>}</svg> : <span className="no-chart">Two samples are needed before a trend line can be drawn.</span>}</div>
}
