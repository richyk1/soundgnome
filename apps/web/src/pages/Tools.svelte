<script lang="ts">
  import { onMount } from 'svelte';
  import { getStorageStats, getSyncSchedules, createSyncSchedule, updateSyncSchedule, deleteSyncSchedule, triggerSyncSchedule, getSoundcloudStatus, connectSoundcloud, disconnectSoundcloud, getSpotifyStatus, connectSpotify, disconnectSpotify, startSpotifyLogin,
    completeSpotifyLogin, logoutSpotify, getSpotifyAudioStatus, connectSpotifyAudio, disconnectSpotifyAudio, downloadUrl } from '../lib/api';
  import type { StorageStatsDto, SyncScheduleDto, SoundcloudStatusDto, SpotifyStatusDto, SpotifyAudioStatusDto } from '../lib/api';

  // ── Tab ────────────────────────────────────────────────────────────────────
  type Tab = 'storage' | 'sync' | 'providers';

  let {
    initialTab = 'sync',
    spotifyMessage = null,
    spotifyCode = null,
  }: {
    initialTab?: Tab;
    spotifyMessage?: string | null;
    /// Authorization code from the Spotify redirect, exchanged on mount.
    spotifyCode?: { code: string; state: string } | null;
  } = $props();

  let activeTab: Tab = $state(initialTab);

  // ── Storage ─────────────────────────────────────────────────────────────────
  let stats: StorageStatsDto | null = $state(null);
  let storageLoading = $state(true);
  let storageError: string | null = $state(null);

  async function loadStorage() {
    storageLoading = true;
    storageError = null;
    try {
      stats = await getStorageStats();
    } catch (err: unknown) {
      storageError = err instanceof Error ? err.message : String(err);
    } finally {
      storageLoading = false;
    }
  }

  function formatBytes(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unitIdx = 0;
    while (size >= 1024 && unitIdx < units.length - 1) {
      size /= 1024;
      unitIdx += 1;
    }
    if (unitIdx === 0) return `${bytes} ${units[0]}`;
    return `${size.toFixed(1)} ${units[unitIdx]}`;
  }

  // ── Sync ─────────────────────────────────────────────────────────────────────
  let schedules: SyncScheduleDto[] = $state([]);
  let syncLoading = $state(true);
  let syncError: string | null = $state(null);

  // Create form
  let newUrl = $state('');
  let newLabel = $state('');
  let newScheduleType = $state<'interval' | 'cron'>('interval');
  let newIntervalHours = $state(1);
  let newCronExpression = $state('0 12 * * *');
  let creating = $state(false);
  let createError: string | null = $state(null);

  let triggeringId: number | null = $state(null);
  let triggerMsg: string | null = $state(null);

  async function loadSync() {
    syncLoading = true;
    syncError = null;
    try {
      schedules = await getSyncSchedules();
    } catch (e: unknown) {
      syncError = e instanceof Error ? e.message : String(e);
    } finally {
      syncLoading = false;
    }
  }

  async function handleCreate(e: SubmitEvent) {
    e.preventDefault();
    if (!newUrl.trim()) return;
    creating = true;
    createError = null;
    try {
      const body: any = {
        playlist_url: newUrl.trim(),
        label: newLabel.trim() || null,
      };
      if (newScheduleType === 'interval') {
        body.interval_hours = newIntervalHours;
      } else {
        body.cron_expression = newCronExpression;
      }
      await createSyncSchedule(body);
      newUrl = '';
      newLabel = '';
      newIntervalHours = 1;
      newCronExpression = '0 12 * * *';
      newScheduleType = 'interval';
      await loadSync();
    } catch (e: unknown) {
      createError = e instanceof Error ? e.message : String(e);
    } finally {
      creating = false;
    }
  }

  async function toggleEnabled(schedule: SyncScheduleDto) {
    try {
      const updated = await updateSyncSchedule(schedule.id, { enabled: !schedule.enabled });
      schedules = schedules.map((s) => (s.id === schedule.id ? updated : s));
    } catch (e: unknown) {
      syncError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleDelete(id: number) {
    if (!confirm('Delete this sync schedule?')) return;
    try {
      await deleteSyncSchedule(id);
      schedules = schedules.filter((s) => s.id !== id);
    } catch (e: unknown) {
      syncError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleTrigger(id: number) {
    triggeringId = id;
    triggerMsg = null;
    try {
      const res = await triggerSyncSchedule(id);
      triggerMsg = `Sync started (task #${res.task_id})`;
      await loadSync();
    } catch (e: unknown) {
      triggerMsg = e instanceof Error ? e.message : String(e);
    } finally {
      triggeringId = null;
      setTimeout(() => (triggerMsg = null), 5000);
    }
  }

  function formatSchedule(schedule: SyncScheduleDto): string {
    if (schedule.interval_hours !== null && schedule.interval_hours !== undefined) {
      const h = schedule.interval_hours;
      if (h < 1) return `every ${Math.round(h * 60)}m`;
      if (h === 1) return 'every hour';
      if (h === Math.floor(h)) return `every ${Math.floor(h)}h`;
      return `every ${h}h`;
    }
    return `cron: ${schedule.cron_expression || '?'}`;
  }

  function formatDate(dt: string | null): string {
    if (!dt) return '—';
    const d = new Date(dt.replace(' ', 'T'));
    return d.toLocaleString();
  }

  onMount(async () => {
    loadStorage();
    loadSync();
    loadSoundcloud();
    loadSpotifyAudio();

    // A redirect from Spotify carries the authorization code. Exchange it
    // before reading the status, otherwise the card renders as logged out and
    // then flips a moment later.
    if (spotifyCode) {
      spLoading = true;
      try {
        spStatus = await completeSpotifyLogin(spotifyCode.code, spotifyCode.state);
        spSuccess = 'Spotify account connected.';
      } catch (e: unknown) {
        spLoginError = e instanceof Error ? e.message : String(e);
        await loadSpotify();
      } finally {
        spLoading = false;
      }
      return;
    }

    loadSpotify();
  });

  function switchTab(tab: Tab) {
    activeTab = tab;
  }

  // ── SoundCloud ───────────────────────────────────────────────────────────────
  let scStatus: SoundcloudStatusDto | null = $state(null);
  let scLoading = $state(true);
  let scToken = $state('');
  let scPending = $state(false);
  let scError: string | null = $state(null);
  let scSuccess: string | null = $state(null);

  async function loadSoundcloud() {
    scLoading = true;
    scError = null;
    try {
      scStatus = await getSoundcloudStatus();
    } catch (e: unknown) {
      scError = e instanceof Error ? e.message : String(e);
    } finally {
      scLoading = false;
    }
  }

  async function handleConnect(e: SubmitEvent) {
    e.preventDefault();
    const token = scToken.trim();
    if (!token || scPending) return;
    scPending = true;
    scError = null;
    scSuccess = null;
    try {
      scStatus = await connectSoundcloud(token);
      scToken = '';
      scSuccess = 'SoundCloud account connected.';
    } catch (e: unknown) {
      scError = e instanceof Error ? e.message : String(e);
    } finally {
      scPending = false;
    }
  }

  async function handleDisconnect() {
    scPending = true;
    scError = null;
    scSuccess = null;
    try {
      scStatus = await disconnectSoundcloud();
      scToken = '';
    } catch (e: unknown) {
      scError = e instanceof Error ? e.message : String(e);
    } finally {
      scPending = false;
    }
  }

  // SoundCloud's own likes URL. The backend recognises it and routes it
  // through the normal playlist sync, so this reuses the download endpoint.
  const SOUNDCLOUD_LIKES_URL = 'https://soundcloud.com/you/likes';

  // Spotify's own Liked Songs URL. The backend recognises it and routes it
  // through the normal playlist sync.
  const SPOTIFY_LIKED_URL = 'https://open.spotify.com/collection/tracks';

  async function handleSpotifySyncLikes() {
    spPending = true;
    spError = null;
    spSuccess = null;
    try {
      await downloadUrl(SPOTIFY_LIKED_URL);
      spSuccess = 'Likes sync started. Follow its progress on the Tasks page.';
    } catch (e: unknown) {
      spError = e instanceof Error ? e.message : String(e);
    } finally {
      spPending = false;
    }
  }

  async function handleSyncLikes() {
    scPending = true;
    scError = null;
    scSuccess = null;
    try {
      await downloadUrl(SOUNDCLOUD_LIKES_URL);
      scSuccess = 'Likes sync started. Follow its progress on the Tasks page.';
    } catch (e: unknown) {
      scError = e instanceof Error ? e.message : String(e);
    } finally {
      scPending = false;
    }
  }

  // ── Spotify ──────────────────────────────────────────────────────────────────
  let spStatus: SpotifyStatusDto | null = $state(null);
  let spLoading = $state(true);
  let spClientId = $state('');
  let spClientSecret = $state('');
  let spPending = $state(false);
  let spError: string | null = $state(null);
  let spSuccess: string | null = $state(null);
  // Login errors survive a status refresh, so the message from the OAuth redirect stays visible.
  let spLoginPending = $state(false);
  let spLoginError: string | null = $state(spotifyMessage);

  async function loadSpotify() {
    spLoading = true;
    spError = null;
    try {
      spStatus = await getSpotifyStatus();
    } catch (e: unknown) {
      spError = e instanceof Error ? e.message : String(e);
    } finally {
      spLoading = false;
    }
  }

  async function handleSpotifyConnect(e: SubmitEvent) {
    e.preventDefault();
    const clientId = spClientId.trim();
    const clientSecret = spClientSecret.trim();
    if (!clientId || !clientSecret || spPending) return;
    spPending = true;
    spError = null;
    spSuccess = null;
    try {
      spStatus = await connectSpotify(clientId, clientSecret);
      spClientId = '';
      spClientSecret = '';
      spSuccess = 'Spotify app credentials saved.';
    } catch (e: unknown) {
      spError = e instanceof Error ? e.message : String(e);
    } finally {
      spPending = false;
    }
  }

  async function handleSpotifyDisconnect() {
    spPending = true;
    spError = null;
    spSuccess = null;
    spLoginError = null;
    try {
      spStatus = await disconnectSpotify();
      spClientId = '';
      spClientSecret = '';
    } catch (e: unknown) {
      spError = e instanceof Error ? e.message : String(e);
    } finally {
      spPending = false;
    }
  }

  async function handleSpotifyLogin() {
    if (spLoginPending) return;
    spLoginPending = true;
    spLoginError = null;
    try {
      window.location.href = await startSpotifyLogin();
    } catch (e: unknown) {
      spLoginError = e instanceof Error ? e.message : String(e);
      spLoginPending = false;
    }
  }

  async function handleSpotifyLogout() {
    if (spLoginPending) return;
    spLoginPending = true;
    spLoginError = null;
    try {
      spStatus = await logoutSpotify();
    } catch (e: unknown) {
      spLoginError = e instanceof Error ? e.message : String(e);
    } finally {
      spLoginPending = false;
    }
  }

  // ── Spotify audio (librespot) ─────────────────────────────────────────────────
  let spaStatus: SpotifyAudioStatusDto | null = $state(null);
  let spaLoading = $state(true);
  let spaPending = $state(false);
  let spaError: string | null = $state(null);
  let spaSuccess: string | null = $state(null);

  async function loadSpotifyAudio() {
    spaLoading = true;
    spaError = null;
    try {
      spaStatus = await getSpotifyAudioStatus();
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaLoading = false;
    }
  }

  async function handleSpotifyAudioConnect() {
    if (spaPending) return;
    spaPending = true;
    spaError = null;
    spaSuccess = null;
    try {
      const authorizeUrl = await connectSpotifyAudio();
      // Approval happens in Spotify, the server catches the callback, so poll
      // until the credentials land rather than blocking the request.
      window.open(authorizeUrl, '_blank', 'noopener');
      spaSuccess = 'Approve the Spotify tab, then this card updates on its own.';
      await pollSpotifyAudio();
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaPending = false;
    }
  }

  /// Watch for the background callback to complete, for up to five minutes.
  async function pollSpotifyAudio() {
    for (let i = 0; i < 100; i++) {
      await new Promise(resolve => setTimeout(resolve, 3000));
      try {
        const status = await getSpotifyAudioStatus();
        if (status.connected) {
          spaStatus = status;
          spaSuccess = `Spotify audio connected as ${status.username ?? 'your account'}.`;
          return;
        }
      } catch {
        // A transient failure while waiting is not worth surfacing.
      }
    }
    spaError = 'Authorization did not complete. Try connecting again.';
  }

  async function handleSpotifyAudioDisconnect() {
    spaPending = true;
    spaError = null;
    spaSuccess = null;
    try {
      spaStatus = await disconnectSpotifyAudio();
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaPending = false;
    }
  }
</script>

<div class="tools-page">
  <div class="page-header">
    <h1>Tools</h1>
  </div>

  <div class="tabs">
    <button class="tab" class:active={activeTab === 'sync'} onclick={() => switchTab('sync')}>
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/>
      </svg>
      Sync
    </button>
    <button class="tab" class:active={activeTab === 'storage'} onclick={() => switchTab('storage')}>
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <ellipse cx="12" cy="5" rx="9" ry="3"/>
        <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/>
        <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
      </svg>
      Storage
    </button>
    <button class="tab" class:active={activeTab === 'providers'} onclick={() => switchTab('providers')}>
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
        <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
      </svg>
      Providers
    </button>
  </div>

  <!-- ── Storage tab ─────────────────────────────────────────────────────────── -->
  {#if activeTab === 'storage'}
    <div class="tab-content">
      <div class="section-toolbar">
        <button class="btn-header" onclick={loadStorage} disabled={storageLoading}>
          {storageLoading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      {#if storageError}
        <div class="feedback error"><strong>Error:</strong> {storageError}</div>
      {/if}

      {#if storageLoading}
        <div class="status">Loading storage statistics…</div>
      {:else if stats}
        <div class="stats-header">
          <div class="total-size">
            <span class="stat-label">Library Size</span>
            <span class="stat-value">{stats.total_formatted}</span>
            <span class="stat-bytes">({stats.total_bytes.toLocaleString()} bytes)</span>
          </div>
          <div class="stat-aside">{stats.artists.length} artists</div>
        </div>

        {#if stats.artists.length === 0}
          <p class="status">No artists or storage data available.</p>
        {:else}
          <ul class="artists-list">
            {#each stats.artists as artist (artist.id)}
              <li class="artist-row">
                <div class="artist-name">{artist.name}</div>
                <div class="artist-bar">
                  <div class="bar-track">
                    <div
                      class="bar-fill"
                      style="width: {Math.max(artist.percent, 2)}%"
                      title="{artist.name}: {artist.percent.toFixed(1)}% ({formatBytes(artist.bytes)})"
                    ></div>
                  </div>
                </div>
                <div class="artist-meta">
                  <span class="meta-pct">{artist.percent.toFixed(1)}%</span>
                  <span class="meta-size">{formatBytes(artist.bytes)}</span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </div>
  {/if}

  <!-- ── Sync tab ────────────────────────────────────────────────────────────── -->
  {#if activeTab === 'sync'}
    <div class="tab-content">
      <p class="sync-subtitle">Define playlists to synchronize automatically using intervals or cron expressions.</p>

      <section class="create-section">
        <h3>Add a schedule</h3>
        <form class="create-form" onsubmit={handleCreate}>
          <div class="form-row">
            <input
              type="url"
              placeholder="Playlist URL (Spotify, SoundCloud, YouTube…)"
              bind:value={newUrl}
              disabled={creating}
              required
            />
          </div>
          <div class="form-row form-row--split">
            <input
              type="text"
              placeholder="Label (optional)"
              bind:value={newLabel}
              disabled={creating}
            />
            <div class="schedule-type-toggle">
              <button
                type="button"
                class="toggle-btn"
                class:active={newScheduleType === 'interval'}
                disabled={creating}
                onclick={() => (newScheduleType = 'interval')}
              >Interval</button>
              <button
                type="button"
                class="toggle-btn"
                class:active={newScheduleType === 'cron'}
                disabled={creating}
                onclick={() => (newScheduleType = 'cron')}
              >Cron</button>
            </div>
          </div>

          {#if newScheduleType === 'interval'}
            <div class="form-row form-row--split">
              <div class="interval-group">
                <input type="number" min="0.25" step="0.25" bind:value={newIntervalHours} disabled={creating} />
                <span class="interval-hint">hours</span>
              </div>
              <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
                {#if creating}<span class="spinner"></span> Adding…{:else}Add{/if}
              </button>
            </div>
          {:else}
            <div class="form-row form-row--split">
              <input type="text" placeholder="Cron expression (e.g. '0 12 * * *' for daily at noon)" bind:value={newCronExpression} disabled={creating} />
              <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
                {#if creating}<span class="spinner"></span> Adding…{:else}Add{/if}
              </button>
            </div>
          {/if}

          {#if createError}
            <p class="feedback error">{createError}</p>
          {/if}
        </form>
      </section>

      {#if triggerMsg}
        <div class="feedback info">{triggerMsg}</div>
      {/if}
      {#if syncError}
        <div class="feedback error">{syncError}</div>
      {/if}

      {#if syncLoading}
        <p class="status">Loading…</p>
      {:else if schedules.length === 0}
        <p class="status">No schedules yet.</p>
      {:else}
        <ul class="schedule-list">
          {#each schedules as schedule (schedule.id)}
            <li class="schedule-card" class:disabled={!schedule.enabled}>
              <div class="schedule-header">
                <div class="schedule-info">
                  <span class="schedule-label">{schedule.label ?? schedule.playlist_url}</span>
                  {#if schedule.label}
                    <span class="schedule-url">{schedule.playlist_url}</span>
                  {/if}
                </div>
                <div class="schedule-meta">
                  <span class="interval-badge">{formatSchedule(schedule)}</span>
                  <span class="status-badge" class:enabled={schedule.enabled} class:paused={!schedule.enabled}>
                    {schedule.enabled ? 'Active' : 'Paused'}
                  </span>
                </div>
              </div>
              <div class="schedule-dates">
                <span>Last run: {formatDate(schedule.last_run)}</span>
                <span>Next run: {formatDate(schedule.next_run)}</span>
              </div>
              <div class="schedule-actions">
                <button class="btn-secondary" onclick={() => toggleEnabled(schedule)}>
                  {schedule.enabled ? 'Pause' : 'Resume'}
                </button>
                <button class="btn-accent" disabled={triggeringId === schedule.id} onclick={() => handleTrigger(schedule.id)}>
                  {#if triggeringId === schedule.id}
                    <span class="spinner"></span> Syncing…
                  {:else}
                    Sync now
                  {/if}
                </button>
                <button class="btn-danger" onclick={() => handleDelete(schedule.id)}>Delete</button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}

  <!-- ── Providers tab ───────────────────────────────────────────────────────── -->
  {#if activeTab === 'providers'}
    <div class="tab-content">
      <p class="sync-subtitle">
        Connect external accounts so downloads and metadata can use your own access.
      </p>

      <section class="schedule-card">
        <div class="schedule-header">
          <div class="schedule-info">
            <span class="schedule-label">SoundCloud connection</span>
          </div>
          {#if !scLoading}
            <div class="schedule-meta">
              <span
                class="status-badge"
                class:enabled={scStatus?.connected}
                class:paused={!scStatus?.connected}
              >
                {scStatus?.connected ? 'Connected' : 'Not connected'}
              </span>
            </div>
          {/if}
        </div>

        <p class="provider-note">
          Connecting unlocks SoundCloud downloadable originals, often FLAC, plus age and region gated
          tracks. Availability is per track and depends on the uploader enabling downloads.
        </p>

        {#if scError}
          <p class="feedback error">{scError}</p>
        {/if}
        {#if scSuccess}
          <p class="feedback info">{scSuccess}</p>
        {/if}

        {#if scLoading}
          <p class="provider-note">Loading…</p>
        {:else if scStatus?.connected}
          <p class="provider-note">
            Connected as <strong>{scStatus.username ?? 'unknown account'}</strong>
          </p>
          <p class="provider-note">
            Syncing your likes creates a playlist called SoundCloud Likes and downloads every liked
            track that is not already in your library. It runs as a background task, so you can
            follow it on the Tasks page.
          </p>
          <div class="schedule-actions">
            <button class="btn-accent" disabled={scPending} onclick={handleSyncLikes}>
              {#if scPending}<span class="spinner"></span> Working…{:else}Sync my likes{/if}
            </button>
            <button class="btn-danger" disabled={scPending} onclick={handleDisconnect}>
              {#if scPending}<span class="spinner"></span> Disconnecting…{:else}Disconnect{/if}
            </button>
          </div>
        {:else}
          <form class="form-row form-row--split" onsubmit={handleConnect}>
            <input
              type="password"
              placeholder="Paste your oauth_token cookie value"
              bind:value={scToken}
              disabled={scPending}
              autocomplete="off"
              spellcheck="false"
            />
            <button type="submit" class="btn-accent" disabled={scPending || !scToken.trim()}>
              {#if scPending}<span class="spinner"></span> Connecting…{:else}Connect{/if}
            </button>
          </form>
          <p class="provider-note">
            Where to find it: log in to soundcloud.com, open your browser devtools, go to Application
            or Storage, then Cookies, then soundcloud.com, and copy the value of the oauth_token
            cookie.
          </p>
        {/if}
      </section>

      <section class="schedule-card">
        <div class="schedule-header">
          <div class="schedule-info">
            <span class="schedule-label">Spotify connection</span>
          </div>
          {#if !spLoading}
            <div class="schedule-meta">
              <span
                class="status-badge"
                class:enabled={spStatus?.connected}
                class:paused={!spStatus?.connected}
              >
                {spStatus?.connected ? 'Connected' : 'Not connected'}
              </span>
            </div>
          {/if}
        </div>

        <p class="provider-note">
          Spotify is used to enrich metadata such as cover art, release date and track numbers. It
          never supplies audio.
        </p>

        {#if spError}
          <p class="feedback error">{spError}</p>
        {/if}
        {#if spSuccess}
          <p class="feedback info">{spSuccess}</p>
        {/if}
        {#if spLoginError}
          <p class="feedback error">{spLoginError}</p>
        {/if}

        {#if spLoading}
          <p class="provider-note">Loading…</p>
        {:else if spStatus?.connected}
          <p class="provider-note">
            Connected as app <strong>{spStatus.client_id ?? 'unknown app'}</strong>
          </p>
          {#if spStatus.user_connected}
            <p class="provider-note">
              Logged in as <strong>{spStatus.user_name ?? 'your Spotify account'}</strong>
            </p>
          {/if}
          <p class="provider-note">
            Logging in lets Soundome read your Liked Songs. It does not download anything.
          </p>
          <div class="schedule-actions">
            {#if spStatus.user_connected}
              <button class="btn-accent" disabled={spPending} onclick={handleSpotifySyncLikes}>
                {#if spPending}<span class="spinner"></span> Working…{:else}Sync my likes{/if}
              </button>
              <button class="btn-secondary" disabled={spLoginPending} onclick={handleSpotifyLogout}>
                {#if spLoginPending}<span class="spinner"></span> Logging out…{:else}Log out{/if}
              </button>
            {:else}
              <button class="btn-accent" disabled={spLoginPending} onclick={handleSpotifyLogin}>
                {#if spLoginPending}<span class="spinner"></span> Redirecting…{:else}Log in with Spotify{/if}
              </button>
            {/if}
            <button class="btn-danger" disabled={spPending} onclick={handleSpotifyDisconnect}>
              {#if spPending}<span class="spinner"></span> Disconnecting…{:else}Disconnect{/if}
            </button>
          </div>
        {:else}
          <form class="form-row form-row--split" onsubmit={handleSpotifyConnect}>
            <input
              class="credential-input"
              type="text"
              placeholder="Client id"
              bind:value={spClientId}
              disabled={spPending}
              autocomplete="off"
              spellcheck="false"
            />
            <input
              class="credential-input"
              type="password"
              placeholder="Client secret"
              bind:value={spClientSecret}
              disabled={spPending}
              autocomplete="off"
              spellcheck="false"
            />
            <button
              type="submit"
              class="btn-accent"
              disabled={spPending || !spClientId.trim() || !spClientSecret.trim()}
            >
              {#if spPending}<span class="spinner"></span> Connecting…{:else}Connect{/if}
            </button>
          </form>
          <p class="provider-note">
            Where to find them: create an app at developer.spotify.com/dashboard, set any redirect
            URI, then copy the client id and the client secret.
          </p>
        {/if}
      </section>

      <section class="schedule-card">
        <div class="schedule-header">
          <div class="schedule-info">
            <span class="schedule-label">Spotify audio</span>
          </div>
          {#if !spaLoading}
            <div class="schedule-meta">
              <span
                class="status-badge"
                class:enabled={spaStatus?.connected}
                class:paused={!spaStatus?.connected}
              >
                {spaStatus?.connected ? 'Connected' : 'Not connected'}
              </span>
            </div>
          {/if}
        </div>

        <p class="provider-note">
          Direct-download session over librespot. Requires Spotify Premium. Once connected, Spotify
          tracks download from Spotify itself as 320 kbps Ogg Vorbis, kept as is with no re-encode,
          instead of being matched on YouTube.
        </p>
        <p class="provider-note">
          Connecting opens a Spotify tab. Approve it and this card updates on its own. Only one
          authorization can be in flight at a time; a pending one expires after five minutes.
        </p>

        {#if spaError}
          <p class="feedback error">{spaError}</p>
        {/if}
        {#if spaSuccess}
          <p class="feedback info">{spaSuccess}</p>
        {/if}

        {#if spaLoading}
          <p class="provider-note">Loading…</p>
        {:else if spaStatus?.connected}
          <p class="provider-note">
            Connected as <strong>{spaStatus.username ?? 'your Spotify account'}</strong>
          </p>
          <div class="schedule-actions">
            <button class="btn-danger" disabled={spaPending} onclick={handleSpotifyAudioDisconnect}>
              {#if spaPending}<span class="spinner"></span> Disconnecting…{:else}Disconnect{/if}
            </button>
          </div>
        {:else}
          <div class="schedule-actions">
            <button class="btn-accent" disabled={spaPending} onclick={handleSpotifyAudioConnect}>
              {#if spaPending}<span class="spinner"></span> Authorizing…{:else}Connect Spotify audio{/if}
            </button>
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .tools-page {
    max-width: 900px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }

  .page-header {
    margin-bottom: 1.5rem;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
  }

  /* ── Tabs ──────────────────────────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--border);
    margin-bottom: 1.75rem;
  }

  .tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--muted);
    font-size: 0.875rem;
    font-family: inherit;
    padding: 0.5rem 1.1rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: -1px;
    transition: color 0.15s, border-color 0.15s;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  /* ── Tab content ───────────────────────────────────────────────────────────── */
  .tab-content {
    animation: fadein 0.12s ease;
  }

  @keyframes fadein {
    from { opacity: 0; transform: translateY(4px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  .section-toolbar {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 1.25rem;
  }

  /* ── Storage ───────────────────────────────────────────────────────────────── */
  .stats-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 1.5rem;
    background: var(--surface);
    border-radius: 8px;
    border: 1px solid var(--border);
    margin-bottom: 1.5rem;
  }

  .total-size {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .stat-label {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
    font-weight: 600;
  }

  .stat-value {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text);
  }

  .stat-bytes {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .stat-aside {
    font-size: 0.875rem;
    color: var(--muted);
  }

  .artists-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .artist-row {
    display: grid;
    grid-template-columns: 200px 1fr 120px;
    gap: 1rem;
    align-items: center;
    padding: 0.65rem 0.9rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    transition: background 0.1s;
  }

  .artist-row:hover {
    background: var(--surface);
  }

  .artist-name {
    font-weight: 500;
    font-size: 0.875rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist-bar {
    display: flex;
    align-items: center;
  }

  .bar-track {
    flex: 1;
    height: 5px;
    background: var(--surface-2);
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .artist-meta {
    display: flex;
    justify-content: flex-end;
    gap: 1rem;
    align-items: center;
  }

  .meta-pct {
    font-size: 0.78rem;
    color: var(--muted);
    min-width: 38px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .meta-size {
    font-size: 0.82rem;
    color: var(--text);
    font-weight: 500;
    min-width: 50px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* ── Sync ──────────────────────────────────────────────────────────────────── */
  .sync-subtitle {
    color: var(--muted);
    margin: 0 0 1.5rem;
    font-size: 0.875rem;
  }

  .create-section h3 {
    font-size: 0.875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin: 0 0 0.75rem;
  }

  .create-form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 2rem;
  }

  .form-row {
    display: flex;
  }

  .form-row--split {
    gap: 0.5rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .form-row input[type="url"],
  .form-row input[type="password"],
  .form-row input[type="text"] {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
  }

  .form-row input[type="url"]:focus,
  .form-row input[type="password"]:focus,
  .form-row input[type="text"]:focus {
    border-color: var(--accent);
  }

  .form-row input:disabled {
    opacity: 0.5;
  }

  .schedule-type-toggle {
    display: flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }

  .toggle-btn {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: none;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.875rem;
    font-weight: 500;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .toggle-btn:not(:last-child) {
    border-right: 1px solid var(--border);
  }

  .toggle-btn.active {
    background: var(--accent);
    color: #fff;
  }

  .toggle-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .interval-group {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .interval-group input[type="number"] {
    width: 80px;
    padding: 0.5rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
  }

  .interval-group input[type="number"]:focus {
    border-color: var(--accent);
  }

  .interval-group input[type="number"]:disabled {
    opacity: 0.5;
  }

  .interval-hint {
    font-size: 0.8rem;
    color: var(--muted);
    white-space: nowrap;
  }

  .schedule-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .schedule-card {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1rem 1.1rem;
    background: var(--surface);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .schedule-card.disabled {
    opacity: 0.6;
  }

  .schedule-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }

  .schedule-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .schedule-label {
    font-weight: 600;
    font-size: 0.9rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .schedule-url {
    font-size: 0.75rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .schedule-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .interval-badge {
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 20px;
    background: var(--surface-2);
    color: var(--muted);
  }

  .status-badge {
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    border-radius: 20px;
    font-weight: 600;
  }

  .status-badge.enabled {
    background: rgba(100, 200, 120, 0.15);
    color: #6dc87a;
  }

  .status-badge.paused {
    background: rgba(180, 180, 180, 0.08);
    color: var(--muted);
  }

  .schedule-dates {
    display: flex;
    gap: 1.5rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .schedule-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  /* ── Shared buttons ────────────────────────────────────────────────────────── */
  .btn-header {
    padding: 0.4rem 1rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    font-size: 0.875rem;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.15s;
  }

  .btn-header:hover:not(:disabled) {
    background: var(--surface-2);
  }

  .btn-header:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-accent {
    padding: 0.5rem 1.1rem;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.875rem;
    font-weight: 500;
    font-family: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    transition: opacity 0.15s;
  }

  .btn-accent:hover:not(:disabled) {
    opacity: 0.88;
  }

  .btn-accent:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    padding: 0.5rem 1.1rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.875rem;
    font-family: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    transition: background 0.15s;
  }

  .btn-secondary:hover {
    background: var(--surface);
  }

  .btn-danger {
    padding: 0.5rem 1.1rem;
    border-radius: 6px;
    border: 1px solid var(--error);
    background: transparent;
    color: var(--error);
    font-size: 0.875rem;
    font-family: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    transition: background 0.15s;
  }

  .btn-danger:hover {
    background: color-mix(in srgb, var(--error) 12%, transparent);
  }

  /* ── Feedback ──────────────────────────────────────────────────────────────── */
  .feedback {
    padding: 0.6rem 0.9rem;
    border-radius: 6px;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .feedback.error {
    background: color-mix(in srgb, var(--error) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--error) 35%, transparent);
    color: var(--error);
  }

  .feedback.info {
    background: rgba(100, 200, 120, 0.12);
    color: #6dc87a;
  }

  /* ── Misc ──────────────────────────────────────────────────────────────────── */
  .status {
    text-align: center;
    color: var(--muted);
    padding: 2.5rem 0;
    font-size: 0.875rem;
  }

  .spinner {
    display: inline-block;
    width: 11px;
    height: 11px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @media (max-width: 768px) {
    .stats-header {
      flex-direction: column;
      gap: 1rem;
    }

    .artist-row {
      grid-template-columns: 1fr;
      gap: 0.4rem;
    }

    .artist-meta {
      justify-content: space-between;
    }
  }

  /* ── Providers ─────────────────────────────────────────────────────────────── */
  .provider-note {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.5;
    color: var(--muted);
  }

  .provider-note strong {
    color: var(--text);
  }

  .schedule-card .feedback {
    margin: 0;
  }

  /* Providers stack their cards directly in the tab, without a .schedule-list wrapper. */
  .tab-content > .schedule-card + .schedule-card {
    margin-top: 0.65rem;
  }

  .form-row .credential-input {
    min-width: 12rem;
  }
</style>
