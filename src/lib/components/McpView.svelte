<script lang="ts">
	import { db, mcp, system, type McpStatus } from '$lib/api';
	import { Button } from './index';
	import { onMount, onDestroy } from 'svelte';

	let status = $state<McpStatus>({ running: false, port: 3839, dbPath: null, mcpScriptPath: null });
	let port = $state(3839);
	let error = $state('');
	let hostname = $state('localhost');
	let pollTimer: ReturnType<typeof setInterval> | null = null;

	onMount(async () => {
		await refreshStatus();
		pollTimer = setInterval(refreshStatus, 3000);
		try {
			const saved = localStorage.getItem('mcpHostname');
			if (saved) hostname = saved;
		} catch {}
	});

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
	});

	async function refreshStatus() {
		try {
			status = await mcp.status();
			if (status.running) {
				port = status.port;
			}
			error = '';
		} catch (e) {
			error = String(e);
		}
	}

	async function handleOpenDb() {
		error = '';
		try {
			const path = await db.browse();
			if (path) {
				await db.open(path);
				localStorage.setItem('lastDbPath', path);
				await refreshStatus();
			}
		} catch (e) {
			error = String(e);
		}
	}

	async function handleStart() {
		error = '';
		try {
			status = await mcp.start(port);
		} catch (e) {
			error = String(e);
		}
	}

	async function handleStop() {
		error = '';
		try {
			await mcp.stop();
			status = { ...status, running: false };
		} catch (e) {
			error = String(e);
		}
	}

	function handleHostnameChange(e: Event) {
		hostname = (e.target as HTMLInputElement).value;
		localStorage.setItem('mcpHostname', hostname);
	}

	async function copyText(text: string) {
		try {
			await system.copyToClipboard(text);
		} catch {
			await navigator.clipboard.writeText(text);
		}
	}

	let skillSaved = $state('');

	async function handleSaveSkill() {
		error = '';
		skillSaved = '';
		try {
			const path = await mcp.saveSkill();
			if (path) {
				skillSaved = path;
			}
		} catch (e) {
			error = String(e);
		}
	}

	let sseUrl = $derived(`http://${hostname}:${port}/sse`);
	let localStdioCmd = $derived(
		status.dbPath && status.mcpScriptPath
			? `claude mcp add sfxroot -- node ${status.mcpScriptPath} ${status.dbPath}`
			: null
	);
	let remoteCmd = $derived(
		`claude mcp add sfxroot --transport sse ${sseUrl}`
	);
</script>

<div class="mcp-view">
	<header class="view-header">
		<h2 class="text-xl font-semibold">MCP Server</h2>
		<div class="status-badge" class:running={status.running}>
			{status.running ? 'Running' : 'Stopped'}
		</div>
	</header>

	{#if !status.dbPath}
		<div class="section">
			<p class="mb-3">Open a database to start the MCP server.</p>
			<Button onclick={handleOpenDb} variant="primary">Open Database</Button>
		</div>
	{:else}
		<div class="section">
			<div class="db-path">
				<span class="label">Database:</span>
				<span class="mono">{status.dbPath}</span>
				<Button onclick={handleOpenDb} variant="ghost" size="sm">Change</Button>
			</div>
		</div>

		<div class="section">
			<div class="port-row">
				<label class="label" for="mcp-port">Port:</label>
				<input
					id="mcp-port"
					type="number"
					class="port-input"
					bind:value={port}
					disabled={status.running}
					min="1024"
					max="65535"
				/>
				{#if status.running}
					<Button onclick={handleStop} variant="secondary" size="sm">Stop</Button>
				{:else}
					<Button onclick={handleStart} variant="primary" size="sm">Start</Button>
				{/if}
			</div>
		</div>

		{#if error}
			<div class="error">{error}</div>
		{/if}

		{#if status.running}
			<div class="section">
				<div class="hostname-row">
					<label class="label" for="mcp-hostname">Hostname / Tailscale IP:</label>
					<input
						id="mcp-hostname"
						type="text"
						class="hostname-input"
						value={hostname}
						oninput={handleHostnameChange}
						placeholder="localhost or tailscale IP"
					/>
				</div>
			</div>

			<div class="section commands-section">
				<h3 class="section-title">Add to Claude Code</h3>

				<div class="command-block">
					<div class="command-label">Remote / Tailscale (SSE):</div>
					<div class="command-row">
						<code class="command">{remoteCmd}</code>
						<Button onclick={() => copyText(remoteCmd)} variant="ghost" size="sm">Copy</Button>
					</div>
				</div>

				{#if localStdioCmd}
					<div class="command-block">
						<div class="command-label">Local (stdio, no server needed):</div>
						<div class="command-row">
							<code class="command">{localStdioCmd}</code>
							<Button onclick={() => copyText(localStdioCmd!)} variant="ghost" size="sm">Copy</Button>
						</div>
					</div>
				{/if}
			</div>

			<div class="section">
				<h3 class="section-title">Connection Info</h3>
				<div class="info-grid">
					<span class="label">SSE Endpoint:</span>
					<code class="mono">{sseUrl}</code>
					<span class="label">Transport:</span>
					<span>SSE (Server-Sent Events)</span>
					<span class="label">Port:</span>
					<span>{port}</span>
				</div>
			</div>
		{/if}

		<div class="section commands-section">
			<h3 class="section-title">Claude Skill</h3>
			<p class="skill-description">
				Install a Claude Code skill into another project so Claude knows how to search
				and copy sounds from this library. The skill teaches Claude how to use the MCP
				tools — just pick your project folder.
			</p>
			<div class="skill-actions">
				<Button onclick={handleSaveSkill} variant="primary" size="sm">Install Skill to Project</Button>
			</div>
			{#if skillSaved}
				<div class="skill-saved">
					Saved to <code class="mono">{skillSaved}</code>
				</div>
			{/if}
			<div class="skill-hint">
				Places the file at <code class="mono">.claude/skills/sfxroot/SKILL.md</code> in your chosen project.
				Claude Code will auto-discover it — no restart needed.
			</div>
		</div>
	{/if}
</div>

<style>
	.mcp-view {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding: 1.5rem;
		gap: 1rem;
		overflow-y: auto;
	}

	.view-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.status-badge {
		padding: 0.25rem 0.75rem;
		border-radius: 9999px;
		font-size: 0.8125rem;
		font-weight: 600;
		background: #fee2e2;
		color: #991b1b;
	}

	.status-badge.running {
		background: #d1fae5;
		color: #065f46;
	}

	:global(.dark) .status-badge {
		background: #450a0a;
		color: #fca5a5;
	}

	:global(.dark) .status-badge.running {
		background: #064e3b;
		color: #6ee7b7;
	}

	.section {
		padding: 1rem;
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		border-radius: 0.5rem;
	}

	.db-path {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		overflow: hidden;
	}

	.label {
		font-weight: 500;
		white-space: nowrap;
		font-size: 0.875rem;
	}

	.mono {
		font-family: ui-monospace, monospace;
		font-size: 0.8125rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-muted, #64748b);
	}

	.port-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.port-input {
		width: 6rem;
		padding: 0.375rem 0.5rem;
		font-family: ui-monospace, monospace;
		font-size: 0.875rem;
		border: 1px solid var(--color-border);
		border-radius: 0.375rem;
		background: var(--color-bg, #fff);
		color: var(--color-text);
	}

	.hostname-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.hostname-input {
		flex: 1;
		padding: 0.375rem 0.5rem;
		font-family: ui-monospace, monospace;
		font-size: 0.875rem;
		border: 1px solid var(--color-border);
		border-radius: 0.375rem;
		background: var(--color-bg, #fff);
		color: var(--color-text);
	}

	.error {
		padding: 0.75rem 1rem;
		background: #fee2e2;
		color: #991b1b;
		border: 1px solid #fca5a5;
		border-radius: 0.5rem;
		font-size: 0.875rem;
	}

	:global(.dark) .error {
		background: #450a0a;
		color: #fca5a5;
		border-color: #991b1b;
	}

	.commands-section {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.section-title {
		font-size: 0.9375rem;
		font-weight: 600;
		margin-bottom: 0.5rem;
	}

	.command-block {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.command-label {
		font-size: 0.8125rem;
		color: var(--color-text-muted, #64748b);
	}

	.command-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.command {
		flex: 1;
		padding: 0.5rem 0.75rem;
		font-family: ui-monospace, monospace;
		font-size: 0.75rem;
		background: var(--color-bg, #f1f5f9);
		border: 1px solid var(--color-border);
		border-radius: 0.375rem;
		overflow-x: auto;
		white-space: nowrap;
		user-select: all;
	}

	.info-grid {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.375rem 0.75rem;
		align-items: center;
		font-size: 0.875rem;
	}

	.skill-description {
		font-size: 0.8125rem;
		color: var(--color-text-muted, #64748b);
		line-height: 1.5;
	}

	.skill-actions {
		display: flex;
		gap: 0.5rem;
	}

	.skill-saved {
		padding: 0.5rem 0.75rem;
		background: #d1fae5;
		color: #065f46;
		border: 1px solid #6ee7b7;
		border-radius: 0.375rem;
		font-size: 0.8125rem;
	}

	:global(.dark) .skill-saved {
		background: #064e3b;
		color: #6ee7b7;
		border-color: #065f46;
	}

	.skill-hint {
		font-size: 0.75rem;
		color: var(--color-text-muted, #94a3b8);
		line-height: 1.5;
	}
</style>
