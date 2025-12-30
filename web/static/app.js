// DistroVitals Frontend Application

const API_BASE = '/api/v1';

// State
let rankings = [];
let currentDistro = null;

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

    const header = `
        <div class="ranking-row header">
            <span>Rank</span>
            <span>Distribution</span>
            <span>Score</span>
            <span></span>
            <span>Trend</span>
        </div>
    `;

    const rows = rankings.map((d, idx) => {
        const rank = d.rank || idx + 1;
        const rankClass = rank <= 3 ? `rank-${rank}` : '';
        const scoreClass = getScoreClass(d.overall_score);
        const trendIcon = getTrendIcon(d.trend);
        const trendClass = getTrendClass(d.trend);

        return `
            <div class="ranking-row" data-slug="${d.slug}">
                <span class="rank ${rankClass}">#${rank}</span>
                <span class="distro-name">${d.name}</span>
                <span class="score">${d.overall_score.toFixed(1)}</span>
                <span class="score-bar">
                    <div class="score-fill ${scoreClass}" style="width: ${d.overall_score}%"></div>
                </span>
                <span class="trend ${trendClass}">${trendIcon}</span>
            </div>
        `;
    }).join('');

    rankingsTable.innerHTML = `<div class="rankings-list">${header}${rows}</div>`;
    rankingsTable.classList.remove('loading');

    // Add click handlers
    document.querySelectorAll('.ranking-row:not(.header)').forEach(row => {
        row.addEventListener('click', () => showDistroDetail(row.dataset.slug));
    });
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

    distroInfo.innerHTML = `
        <div class="distro-header">
            <div>
                <h2>${distro.name}</h2>
                <span class="trend ${trendClass}" style="font-size: 1.5rem;">${trendIcon} ${distro.trend}</span>
            </div>
            <div class="overall-score ${scoreClass}">${distro.overall_score.toFixed(1)}</div>
        </div>

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

function showRankings() {
    detailSection.classList.add('hidden');
    rankingsSection.classList.remove('hidden');
    currentDistro = null;
}

// Utility functions
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
