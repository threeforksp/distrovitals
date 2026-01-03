// DistroVitals Frontend Application

const API_BASE = '/api/v1';
const PAGE_SIZE = 20;

// SVG Icons
const GITHUB_ICON = `<svg viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>`;
const REDDIT_ICON = `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0zm5.01 4.744c.688 0 1.25.561 1.25 1.249a1.25 1.25 0 0 1-2.498.056l-2.597-.547-.8 3.747c1.824.07 3.48.632 4.674 1.488.308-.309.73-.491 1.207-.491.968 0 1.754.786 1.754 1.754 0 .716-.435 1.333-1.01 1.614a3.111 3.111 0 0 1 .042.52c0 2.694-3.13 4.87-7.004 4.87-3.874 0-7.004-2.176-7.004-4.87 0-.183.015-.366.043-.534A1.748 1.748 0 0 1 4.028 12c0-.968.786-1.754 1.754-1.754.463 0 .898.196 1.207.49 1.207-.883 2.878-1.43 4.744-1.487l.885-4.182a.342.342 0 0 1 .14-.197.35.35 0 0 1 .238-.042l2.906.617a1.214 1.214 0 0 1 1.108-.701zM9.25 12C8.561 12 8 12.562 8 13.25c0 .687.561 1.248 1.25 1.248.687 0 1.248-.561 1.248-1.249 0-.688-.561-1.249-1.249-1.249zm5.5 0c-.687 0-1.248.561-1.248 1.25 0 .687.561 1.248 1.249 1.248.688 0 1.249-.561 1.249-1.249 0-.687-.562-1.249-1.25-1.249zm-5.466 3.99a.327.327 0 0 0-.231.094.33.33 0 0 0 0 .463c.842.842 2.484.913 2.961.913.477 0 2.105-.056 2.961-.913a.361.361 0 0 0 .029-.463.33.33 0 0 0-.464 0c-.547.533-1.684.73-2.512.73-.828 0-1.979-.196-2.512-.73a.326.326 0 0 0-.232-.095z"/></svg>`;

// State
let rankings = [];
let currentDistro = null;
let currentPage = 1;

// DOM Elements
const rankingsSection = document.getElementById('rankings');
const rankingsTable = document.getElementById('rankings-table');
const detailSection = document.getElementById('distro-detail');
const distroInfo = document.getElementById('distro-info');
const backBtn = document.getElementById('back-btn');

// Initialize
document.addEventListener('DOMContentLoaded', init);
backBtn.addEventListener('click', showRankings);

async function init() {
    await loadRankings();
}

// API Functions
async function fetchApi(endpoint) {
    try {
        const response = await fetch(`${API_BASE}${endpoint}`);
        const data = await response.json();
        if (!data.success) {
            throw new Error(data.error || 'API error');
        }
        return data.data;
    } catch (error) {
        console.error('API Error:', error);
        throw error;
    }
}

// Load and display rankings
async function loadRankings() {
    try {
        rankings = await fetchApi('/rankings');
        renderRankings();
    } catch (error) {
        rankingsTable.innerHTML = `
            <p class="error">Failed to load rankings. Is the server running?</p>
            <p class="error-detail">${error.message}</p>
        `;
    }
}

function renderRankings() {
    if (rankings.length === 0) {
        rankingsTable.innerHTML = `
            <p>No health scores available yet.</p>
            <p>Run <code>dv collect all && dv analyze all</code> to collect data.</p>
        `;
        return;
    }

    const totalPages = Math.ceil(rankings.length / PAGE_SIZE);
    const startIdx = (currentPage - 1) * PAGE_SIZE;
    const endIdx = startIdx + PAGE_SIZE;
    const pageRankings = rankings.slice(startIdx, endIdx);

    const header = `
        <div class="ranking-row header">
            <span>Rank</span>
            <span>Distribution</span>
            <span>Score</span>
            <span class="metrics-header">Contributors</span>
            <span class="metrics-header">Releases/30d</span>
            <span class="metrics-header">Stars</span>
            <span>Trend</span>
        </div>
    `;

    const rows = pageRankings.map((d, idx) => {
        const rank = d.rank || (startIdx + idx + 1);
        const rankClass = rank <= 3 ? `rank-${rank}` : '';
        const scoreClass = getScoreClass(d.overall_score);
        const trendIcon = getTrendIcon(d.trend);
        const trendClass = getTrendClass(d.trend);
        const m = d.metrics || {};
        const dataSources = renderDataSources(d);

        return `
            <div class="ranking-row" data-slug="${d.slug}">
                <span class="rank ${rankClass}">#${rank}</span>
                <span class="distro-name-cell">
                    <span class="distro-name">${d.name}</span>
                    ${dataSources}
                </span>
                <span class="score">${d.overall_score.toFixed(1)}</span>
                <span class="metric">${formatNumber(m.total_contributors || 0)}</span>
                <span class="metric">${m.releases_30d || 0}</span>
                <span class="metric">${formatNumber(m.total_stars || 0)}</span>
                <span class="trend ${trendClass}">${trendIcon}</span>
            </div>
        `;
    }).join('');

    const pagination = totalPages > 1 ? `
        <div class="pagination">
            <button class="page-btn" onclick="goToPage(${currentPage - 1})" ${currentPage === 1 ? 'disabled' : ''}>← Prev</button>
            <span class="page-info">Page ${currentPage} of ${totalPages} (${rankings.length} distros)</span>
            <button class="page-btn" onclick="goToPage(${currentPage + 1})" ${currentPage === totalPages ? 'disabled' : ''}>Next →</button>
        </div>
    ` : `<div class="pagination"><span class="page-info">${rankings.length} distributions</span></div>`;

    rankingsTable.innerHTML = `<div class="rankings-list">${header}${rows}</div>${pagination}`;
    rankingsTable.classList.remove('loading');

    // Add click handlers
    document.querySelectorAll('.ranking-row:not(.header)').forEach(row => {
        row.addEventListener('click', () => showDistroDetail(row.dataset.slug));
    });
}

function goToPage(page) {
    const totalPages = Math.ceil(rankings.length / PAGE_SIZE);
    if (page < 1 || page > totalPages) return;
    currentPage = page;
    renderRankings();
    window.scrollTo({ top: 0, behavior: 'smooth' });
}

// Show detail view for a distribution
async function showDistroDetail(slug) {
    const distro = rankings.find(d => d.slug === slug);
    if (!distro) return;

    currentDistro = distro;

    // Fetch additional data
    let healthData = null;
    let history = [];

    try {
        healthData = await fetchApi(`/distros/${slug}/health`);
    } catch (e) {
        // No health data available
    }

    try {
        history = await fetchApi(`/distros/${slug}/history?days=30`);
    } catch (e) {
        // No history available
    }

    renderDistroDetail(distro, healthData, history);

    rankingsSection.classList.add('hidden');
    detailSection.classList.remove('hidden');
}

function renderDistroDetail(distro, healthData, history) {
    const scoreClass = getScoreClass(distro.overall_score);
    const trendIcon = getTrendIcon(distro.trend);
    const trendClass = getTrendClass(distro.trend);
    const m = distro.metrics || {};
    const dataSources = renderDataSources(distro, true);

    distroInfo.innerHTML = `
        <div class="distro-header">
            <div>
                <h2>${distro.name}</h2>
                <span class="trend ${trendClass}" style="font-size: 1.5rem;">${trendIcon} ${distro.trend}</span>
            </div>
            <div class="overall-score ${scoreClass}">${distro.overall_score.toFixed(1)}</div>
        </div>

        ${dataSources}

        <div class="raw-metrics">
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.total_contributors || 0)}</span>
                <span class="metric-label">Contributors</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.commits_30d || 0)}</span>
                <span class="metric-label">Commits (30d)</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${m.releases_30d || 0}</span>
                <span class="metric-label">Releases (30d)</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.total_stars || 0)}</span>
                <span class="metric-label">Stars</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.total_forks || 0)}</span>
                <span class="metric-label">Forks</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.open_issues || 0)}</span>
                <span class="metric-label">Open Issues</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${formatNumber(m.open_prs || 0)}</span>
                <span class="metric-label">Open PRs</span>
            </div>
            <div class="metric-card">
                <span class="metric-value">${m.total_releases || 0}</span>
                <span class="metric-label">Total Releases</span>
            </div>
        </div>

        ${m.latest_release ? `
        <div class="latest-release">
            <span class="release-tag">${m.latest_release}</span>
            <span class="release-age">${m.days_since_release !== null ? formatDaysAgo(m.days_since_release) : ''}</span>
        </div>
        ` : ''}

        <div class="score-breakdown">
            <div class="score-item">
                <h4>Development Activity</h4>
                <div class="value ${getScoreClass(distro.development_score)}">${distro.development_score.toFixed(1)}</div>
                <div class="score-bar">
                    <div class="score-fill ${getScoreClass(distro.development_score)}" style="width: ${distro.development_score}%"></div>
                </div>
            </div>
            <div class="score-item">
                <h4>Community Engagement</h4>
                <div class="value ${getScoreClass(distro.community_score)}">${distro.community_score.toFixed(1)}</div>
                <div class="score-bar">
                    <div class="score-fill ${getScoreClass(distro.community_score)}" style="width: ${distro.community_score}%"></div>
                </div>
            </div>
            <div class="score-item">
                <h4>Maintenance Health</h4>
                <div class="value ${getScoreClass(distro.maintenance_score)}">${distro.maintenance_score.toFixed(1)}</div>
                <div class="score-bar">
                    <div class="score-fill ${getScoreClass(distro.maintenance_score)}" style="width: ${distro.maintenance_score}%"></div>
                </div>
            </div>
        </div>

        ${history.length > 0 ? renderHistory(history) : '<p>No historical data available yet.</p>'}

        ${renderDetailMethodology(distro)}
    `;
}

function renderHistory(history) {
    if (history.length < 2) return '';

    const points = history.map((h, i) => {
        const x = (i / (history.length - 1)) * 100;
        const y = 100 - h.overall_score;
        return `${x},${y}`;
    }).join(' ');

    return `
        <div class="history-chart">
            <h4>30-Day Trend</h4>
            <svg viewBox="0 0 100 100" preserveAspectRatio="none" style="width: 100%; height: 100px; background: var(--bg-secondary); border-radius: var(--radius);">
                <polyline
                    points="${points}"
                    fill="none"
                    stroke="var(--accent)"
                    stroke-width="2"
                    vector-effect="non-scaling-stroke"
                />
            </svg>
        </div>
    `;
}

function renderDetailMethodology(distro) {
    return `
        <div class="detail-methodology">
            <h3 class="collapsible-header" onclick="window.toggleMethodology('detail-methodology-content'); event.stopPropagation();">
                <span class="toggle-icon">▶</span> How This Score Is Calculated
            </h3>
            <div id="detail-methodology-content" class="collapsible-content collapsed">
                <div class="score-explanation">
                    <p><strong>Overall Score</strong> = Development (40%) + Community (30%) + Maintenance (30%)</p>

                    <div class="score-detail">
                        <h4>Development Activity (${distro.development_score.toFixed(1)})</h4>
                        <p>Based on commits and contributors in the last 30 days from GitHub repositories.</p>
                    </div>

                    <div class="score-detail">
                        <h4>Community Engagement (${distro.community_score.toFixed(1)})</h4>
                        <p>Combines GitHub popularity (stars, forks) with Reddit community size and activity.
                        ${distro.subreddit ? `Reddit data from r/${distro.subreddit}.` : 'No Reddit data available.'}</p>
                    </div>

                    <div class="score-detail">
                        <h4>Maintenance Health (${distro.maintenance_score.toFixed(1)})</h4>
                        <p>Measures open issues, open PRs, and recency of last commit. Lower backlogs = higher scores.</p>
                    </div>

                    <p class="methodology-note"><em>Note: Not all distros develop on GitHub. Scores reflect GitHub/Reddit presence, not overall project quality.</em></p>
                </div>
            </div>
        </div>
    `;
}

function showRankings() {
    detailSection.classList.add('hidden');
    rankingsSection.classList.remove('hidden');
    currentDistro = null;
}

// Utility functions
function formatNumber(num) {
    if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
    return num.toString();
}

function formatDaysAgo(days) {
    if (days === 0) return 'today';
    if (days === 1) return 'yesterday';
    if (days < 7) return `${days} days ago`;
    if (days < 30) return `${Math.floor(days / 7)} weeks ago`;
    if (days < 365) return `${Math.floor(days / 30)} months ago`;
    return `${Math.floor(days / 365)} years ago`;
}

function getScoreClass(score) {
    if (score >= 70) return 'score-high';
    if (score >= 40) return 'score-medium';
    return 'score-low';
}

function getTrendIcon(trend) {
    switch (trend) {
        case 'up': return '↑';
        case 'down': return '↓';
        default: return '→';
    }
}

function getTrendClass(trend) {
    switch (trend) {
        case 'up': return 'trend-up';
        case 'down': return 'trend-down';
        default: return 'trend-stable';
    }
}

// Toggle methodology section (exposed globally for onclick)
window.toggleMethodology = function(elementId) {
    const id = elementId || 'methodology-content';
    const content = document.getElementById(id);
    if (!content) return;

    const header = content.previousElementSibling;

    if (content.classList.contains('collapsed')) {
        content.classList.remove('collapsed');
        if (header) header.classList.add('expanded');
    } else {
        content.classList.add('collapsed');
        if (header) header.classList.remove('expanded');
    }
};

// Render data source badges
function renderDataSources(distro, detailed = false) {
    const badges = [];
    const m = distro.metrics || {};

    if (distro.github_org) {
        const url = `https://github.com/${distro.github_org}`;
        const commits30d = m.commits_30d || 0;
        const commits365d = m.commits_365d || 0;
        const c30 = formatNumber(commits30d);
        const c365 = formatNumber(commits365d);
        const label = detailed
            ? `${distro.github_org} (${c30}/30d, ${c365}/yr)`
            : `${c30}/30d · ${c365}/yr`;
        const title = `GitHub: ${distro.github_org} - ${commits30d.toLocaleString()} commits (30 days), ${commits365d.toLocaleString()} commits (year)`;
        badges.push(`<a href="${url}" target="_blank" class="source-badge github" onclick="event.stopPropagation()" title="${title}">${GITHUB_ICON}<span>${label}</span></a>`);
    }

    if (distro.subreddit) {
        const url = `https://reddit.com/r/${distro.subreddit}`;
        const subs = m.reddit_subscribers || 0;
        const subsFormatted = formatNumber(subs);
        const label = detailed ? `r/${distro.subreddit} (${subsFormatted})` : `${subsFormatted}`;
        badges.push(`<a href="${url}" target="_blank" class="source-badge reddit" onclick="event.stopPropagation()" title="Reddit: r/${distro.subreddit} - ${subs.toLocaleString()} subscribers">${REDDIT_ICON}<span>${label}</span></a>`);
    }

    if (badges.length === 0) {
        return detailed ? '<span class="no-sources">No data sources configured</span>' : '';
    }

    return `<div class="${detailed ? 'detail-data-sources' : 'data-sources'}">${badges.join('')}</div>`;
}
