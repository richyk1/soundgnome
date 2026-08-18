<script lang="ts">
  import { onMount } from 'svelte';
  import BackfillPanel from '../lib/BackfillPanel.svelte';
  import MissingFiles from '../lib/MissingFiles.svelte';
  import { getStorageStats, getSyncSchedules, createSyncSchedule, updateSyncSchedule, deleteSyncSchedule, triggerSyncSchedule, getSoundcloudStatus, connectSoundcloud, disconnectSoundcloud,
    getSpotifyAudioStatus, connectSpotifyAudio, completeSpotifyAudio, disconnectSpotifyAudio, downloadUrl, embedArtwork, backfillFingerprints } from '../lib/api';
  import type { StorageStatsDto, SyncScheduleDto, SoundcloudStatusDto, SpotifyAudioStatusDto } from '../lib/api';

  // ── Tab ────────────────────────────────────────────────────────────────────
  type Tab = 'sync' | 'storage' | 'providers' | 'artwork' | 'fingerprints' | 'missing';

  let {
    initialTab = 'sync',
  }: {
    initialTab?: Tab;
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

  // ── Spotify audio (librespot) ─────────────────────────────────────────────────
  let spaStatus: SpotifyAudioStatusDto | null = $state(null);
  let spaLoading = $state(true);
  let spaPending = $state(false);
  let spaError: string | null = $state(null);
  let spaSuccess: string | null = $state(null);
  let spaAuthorizing = $state(false);
  let spaRedirectUrl = $state('');

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

  const SPOTIFY_LIKED_URL = 'https://open.spotify.com/collection/tracks';

  async function handleSpotifyAudioConnect() {
    if (spaPending) return;
    spaPending = true;
    spaError = null;
    spaSuccess = null;
    try {
      const authorizeUrl = await connectSpotifyAudio();
      window.open(authorizeUrl, '_blank', 'noopener');
      spaAuthorizing = true;
      spaSuccess = 'Approve in the Spotify tab, then paste the URL it lands on below.';
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaPending = false;
    }
  }

  async function handleSpotifyAudioComplete() {
    if (spaPending || !spaRedirectUrl.trim()) return;
    spaPending = true;
    spaError = null;
    spaSuccess = null;
    try {
      spaStatus = await completeSpotifyAudio(spaRedirectUrl.trim());
      spaAuthorizing = false;
      spaRedirectUrl = '';
      spaSuccess = `Spotify connected as ${spaStatus.username ?? 'your account'}.`;
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaPending = false;
    }
  }

  async function handleSpotifyAudioSyncLikes() {
    if (spaPending) return;
    spaPending = true;
    spaError = null;
    spaSuccess = null;
    try {
      await downloadUrl(SPOTIFY_LIKED_URL);
      spaSuccess = 'Liked Songs sync started. Follow its progress on the Tasks page.';
    } catch (e: unknown) {
      spaError = e instanceof Error ? e.message : String(e);
    } finally {
      spaPending = false;
    }
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
  <header class="page-header">
    <h1>Tools</h1>
    <p class="lede">
      Connect external accounts, schedule automatic playlist syncs, and keep an eye on how your
      library uses disk.
    </p>
  </header>

  <div class="tabs" role="tablist">
    <button class="tab" class:active={activeTab === 'sync'} onclick={() => switchTab('sync')}>
      <i class="lni lni-repeat-1" aria-hidden="true"></i>Sync
    </button>
    <button class="tab" class:active={activeTab === 'storage'} onclick={() => switchTab('storage')}>
      <i class="lni lni-database-2" aria-hidden="true"></i>Storage
    </button>
    <button class="tab" class:active={activeTab === 'providers'} onclick={() => switchTab('providers')}>
      <i class="lni lni-plug-1" aria-hidden="true"></i>Providers
    </button>
    <button class="tab" class:active={activeTab === 'artwork'} onclick={() => switchTab('artwork')}>
      <i class="lni lni-gallery" aria-hidden="true"></i>Artwork
    </button>
    <button class="tab" class:active={activeTab === 'fingerprints'} onclick={() => switchTab('fingerprints')}>
      <i class="lni lni-fingerprint-1" aria-hidden="true"></i>Fingerprints
    </button>
    <button class="tab" class:active={activeTab === 'missing'} onclick={() => switchTab('missing')}>
      <i class="lni lni-unlink" aria-hidden="true"></i>Missing files
    </button>
  </div>

  <!-- ── Artwork tab ─────────────────────────────────────────────────────────── -->
  {#if activeTab === 'artwork'}
    <section class="tab-content">
      <BackfillPanel
        title="Embed artwork"
        description="Embed cover art into every library file in place so artwork travels with the audio and shows offline. Missing covers are resolved from each track's references (Spotify oEmbed or YouTube thumbnail). Audio is never re-downloaded or moved."
        icon="gallery"
        taskType="EmbedArtworkBackfill"
        okLabel="embedded"
        skipHint="Skipped = no cover art could be resolved for the track, or the audio file is missing from disk."
        note="Runs in the background on the shared task queue, so it is safe to leave and come back to. Files that already have embedded art are refreshed in place."
        start={embedArtwork}
      />
    </section>
  {/if}

  <!-- ── Fingerprints tab ────────────────────────────────────────────────────── -->
  {#if activeTab === 'fingerprints'}
    <section class="tab-content">
      <BackfillPanel
        title="Fingerprint library"
        description="Compute an acoustic fingerprint (Chromaprint) for every library track so re-uploads of songs you already own are recognized and quality-compared, even when their tags differ. This needs to run once for tracks that predate fingerprinting."
        icon="fingerprint-1"
        taskType="FingerprintBackfill"
        okLabel="fingerprinted"
        skipHint="Skipped = the track is already fingerprinted (safe to re-run), or its audio file is missing from disk."
        note="Runs in the background and is idempotent: already-fingerprinted tracks are skipped, so re-running is cheap. Expect roughly a second or two per track."
        start={backfillFingerprints}
      />
    </section>
  {/if}

  <!-- ── Missing files tab ───────────────────────────────────────────────────── -->
  {#if activeTab === 'missing'}
    <section class="tab-content">
      <MissingFiles />
    </section>
  {/if}

  <!-- ── Storage tab ─────────────────────────────────────────────────────────── -->
  {#if activeTab === 'storage'}
    <section class="tab-content">
      <div class="section-head">
        <h2>Library storage</h2>
        <div class="section-actions">
          <button class="btn-ghost btn-sm" onclick={loadStorage} disabled={storageLoading}>
            {#if storageLoading}<span class="spinner"></span>{:else}<i class="lni lni-refresh-circle-1-clockwise" aria-hidden="true"></i>{/if}Refresh
          </button>
        </div>
      </div>

      {#if storageError}
        <div class="callout callout-error" role="alert">
          <i class="lni lni-xmark-circle" aria-hidden="true"></i>
          <div class="callout-body"><strong>Couldn't load storage statistics.</strong><span>{storageError}</span></div>
        </div>
      {/if}

      {#if storageLoading}
        <ul class="artists-list" aria-hidden="true">
          {#each { length: 5 } as _}
            <li class="artist-row skeleton">
              <span class="sk sk-name"></span>
              <span class="sk sk-bar"></span>
              <span class="sk sk-meta"></span>
            </li>
          {/each}
        </ul>
      {:else if stats}
        <div class="storage-summary">
          <div class="summary-item">
            <span class="summary-label">Library size</span>
            <span class="summary-value">{stats.total_formatted}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">Total bytes</span>
            <span class="summary-mono">{stats.total_bytes.toLocaleString()}</span>
          </div>
          <div class="summary-item">
            <span class="summary-label">Artists</span>
            <span class="summary-mono">{stats.artists.length}</span>
          </div>
        </div>

        {#if stats.artists.length === 0}
          <div class="empty">
            <i class="lni lni-database-2" aria-hidden="true"></i>
            <p class="empty-title">No storage data yet</p>
            <p class="empty-hint">Download some tracks and their footprint will show up here.</p>
          </div>
        {:else}
          <ul class="artists-list">
            {#each stats.artists as artist (artist.id)}
              <li class="artist-row">
                <span class="artist-name">{artist.name}</span>
                <div class="bar-track" title="{artist.name}: {artist.percent.toFixed(1)}% ({formatBytes(artist.bytes)})">
                  <div class="bar-fill" style="transform: scaleX({Math.max(artist.percent, 2) / 100})"></div>
                </div>
                <span class="artist-meta">
                  <span class="meta-pct">{artist.percent.toFixed(1)}%</span>
                  <span class="meta-size">{formatBytes(artist.bytes)}</span>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>
  {/if}

  <!-- ── Sync tab ────────────────────────────────────────────────────────────── -->
  {#if activeTab === 'sync'}
    <section class="tab-content">
      <div class="section-head">
        <h2>Add a schedule</h2>
      </div>
      <p class="lede">Define playlists to synchronize automatically using intervals or cron expressions.</p>

      <form class="create-panel" onsubmit={handleCreate}>
        <div class="field">
          <i class="lni lni-cloud-download field-icon" aria-hidden="true"></i>
          <input
            type="url"
            placeholder="Playlist URL (Spotify, SoundCloud, YouTube…)"
            bind:value={newUrl}
            disabled={creating}
            required
          />
        </div>
        <div class="create-row">
          <input
            class="input"
            type="text"
            placeholder="Label (optional)"
            bind:value={newLabel}
            disabled={creating}
          />
          <div class="seg">
            <button
              type="button"
              class="seg-btn"
              class:active={newScheduleType === 'interval'}
              disabled={creating}
              onclick={() => (newScheduleType = 'interval')}
            >Interval</button>
            <button
              type="button"
              class="seg-btn"
              class:active={newScheduleType === 'cron'}
              disabled={creating}
              onclick={() => (newScheduleType = 'cron')}
            >Cron</button>
          </div>
        </div>

        {#if newScheduleType === 'interval'}
          <div class="create-row">
            <div class="interval-group">
              <input type="number" min="0.25" step="0.25" bind:value={newIntervalHours} disabled={creating} />
              <span class="unit">hours</span>
            </div>
            <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
              {#if creating}<span class="spinner"></span>Adding{:else}<i class="lni lni-calendar-plus" aria-hidden="true"></i>Add{/if}
            </button>
          </div>
        {:else}
          <div class="create-row">
            <input class="input" type="text" placeholder="Cron expression (e.g. '0 12 * * *' for daily at noon)" bind:value={newCronExpression} disabled={creating} />
            <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
              {#if creating}<span class="spinner"></span>Adding{:else}<i class="lni lni-calendar-plus" aria-hidden="true"></i>Add{/if}
            </button>
          </div>
        {/if}

        {#if createError}
          <p class="field-error">{createError}</p>
        {/if}
      </form>

      {#if triggerMsg}
        <div class="callout callout-info" role="status">
          <i class="lni lni-repeat-1" aria-hidden="true"></i>
          <div class="callout-body"><span>{triggerMsg}</span></div>
        </div>
      {/if}
      {#if syncError}
        <div class="callout callout-error" role="alert">
          <i class="lni lni-xmark-circle" aria-hidden="true"></i>
          <div class="callout-body"><span>{syncError}</span></div>
        </div>
      {/if}

      <div class="section-head">
        <h2>Schedules{#if !syncLoading}<span class="count">{schedules.length}</span>{/if}</h2>
      </div>

      {#if syncLoading}
        <ul class="schedule-list" aria-hidden="true">
          {#each { length: 3 } as _}
            <li class="schedule-panel skeleton">
              <span class="sk sk-name"></span>
              <span class="sk sk-sub"></span>
            </li>
          {/each}
        </ul>
      {:else if schedules.length === 0}
        <div class="empty">
          <i class="lni lni-calendar-plus" aria-hidden="true"></i>
          <p class="empty-title">No schedules yet</p>
          <p class="empty-hint">Add a playlist above to sync it automatically.</p>
        </div>
      {:else}
        <ul class="schedule-list">
          {#each schedules as schedule (schedule.id)}
            <li class="schedule-panel" class:disabled={!schedule.enabled}>
              <div class="schedule-top">
                <div class="schedule-info">
                  <span class="schedule-label">{schedule.label ?? schedule.playlist_url}</span>
                  {#if schedule.label}
                    <span class="schedule-url">{schedule.playlist_url}</span>
                  {/if}
                </div>
                <div class="schedule-meta">
                  <span class="pill pill-neutral">{formatSchedule(schedule)}</span>
                  <span class="pill" class:pill-success={schedule.enabled} class:pill-muted={!schedule.enabled}>
                    {schedule.enabled ? 'Active' : 'Paused'}
                  </span>
                </div>
              </div>
              <div class="schedule-dates">
                <span>Last run: {formatDate(schedule.last_run)}</span>
                <span>Next run: {formatDate(schedule.next_run)}</span>
              </div>
              <div class="schedule-actions">
                <button class="btn-ghost btn-sm" onclick={() => toggleEnabled(schedule)}>
                  {#if schedule.enabled}<i class="lni lni-pause" aria-hidden="true"></i>Pause{:else}<i class="lni lni-play" aria-hidden="true"></i>Resume{/if}
                </button>
                <button class="btn-accent btn-sm" disabled={triggeringId === schedule.id} onclick={() => handleTrigger(schedule.id)}>
                  {#if triggeringId === schedule.id}
                    <span class="spinner"></span>Syncing
                  {:else}
                    <i class="lni lni-repeat-1" aria-hidden="true"></i>Sync now
                  {/if}
                </button>
                <button class="btn-danger btn-sm" onclick={() => handleDelete(schedule.id)}>
                  <i class="lni lni-trash-3" aria-hidden="true"></i>Delete
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <!-- ── Providers tab ───────────────────────────────────────────────────────── -->
  {#if activeTab === 'providers'}
    <section class="tab-content">
      <div class="section-head">
        <h2>Connected accounts</h2>
      </div>
      <p class="lede">
        Connect external accounts so downloads and metadata can use your own access.
      </p>

      <div class="provider-panel">
        <div class="provider-head">
          <div class="provider-title">
            <i class="lni lni-soundcloud provider-brand" aria-hidden="true"></i>
            <span class="provider-name">SoundCloud</span>
          </div>
          {#if !scLoading}
            <span class="pill" class:pill-success={scStatus?.connected} class:pill-muted={!scStatus?.connected}>
              {scStatus?.connected ? 'Connected' : 'Not connected'}
            </span>
          {/if}
        </div>

        <p class="provider-note">
          Connecting unlocks SoundCloud downloadable originals, often FLAC, plus age and region gated
          tracks. Availability is per track and depends on the uploader enabling downloads.
        </p>

        {#if scError}
          <div class="callout callout-error" role="alert">
            <i class="lni lni-xmark-circle" aria-hidden="true"></i>
            <div class="callout-body"><span>{scError}</span></div>
          </div>
        {/if}
        {#if scSuccess}
          <div class="callout callout-success" role="status">
            <i class="lni lni-check-circle-1" aria-hidden="true"></i>
            <div class="callout-body"><span>{scSuccess}</span></div>
          </div>
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
          <div class="provider-actions">
            <button class="btn-accent" disabled={scPending} onclick={handleSyncLikes}>
              {#if scPending}<span class="spinner"></span>Working{:else}<i class="lni lni-heart" aria-hidden="true"></i>Sync my likes{/if}
            </button>
            <button class="btn-danger" disabled={scPending} onclick={handleDisconnect}>
              {#if scPending}<span class="spinner"></span>Disconnecting{:else}<i class="lni lni-plug-1" aria-hidden="true"></i>Disconnect{/if}
            </button>
          </div>
        {:else}
          <form class="create-row" onsubmit={handleConnect}>
            <input
              class="input"
              type="password"
              placeholder="Paste your oauth_token cookie value"
              bind:value={scToken}
              disabled={scPending}
              autocomplete="off"
              spellcheck="false"
            />
            <button type="submit" class="btn-accent" disabled={scPending || !scToken.trim()}>
              {#if scPending}<span class="spinner"></span>Connecting{:else}<i class="lni lni-link-1-angular-right" aria-hidden="true"></i>Connect{/if}
            </button>
          </form>
          <p class="provider-note">
            Where to find it: log in to soundcloud.com, open your browser devtools, go to Application
            or Storage, then Cookies, then soundcloud.com, and copy the value of the oauth_token
            cookie.
          </p>
        {/if}
      </div>

      <div class="provider-panel">
        <div class="provider-head">
          <div class="provider-title">
            <i class="lni lni-spotify provider-brand" aria-hidden="true"></i>
            <span class="provider-name">Spotify</span>
          </div>
          {#if !spaLoading}
            <span class="pill" class:pill-success={spaStatus?.connected} class:pill-muted={!spaStatus?.connected}>
              {spaStatus?.connected ? 'Connected' : 'Not connected'}
            </span>
          {/if}
        </div>

        <p class="provider-note">
          Requires Spotify Premium. Connecting authorizes one Spotify session used for everything:
          downloading your Liked Songs directly from Spotify, and reading metadata. Connecting opens
          a browser on the server to authorize (works on a localhost install).
        </p>

        {#if spaError}
          <div class="callout callout-error" role="alert">
            <i class="lni lni-xmark-circle" aria-hidden="true"></i>
            <div class="callout-body"><span>{spaError}</span></div>
          </div>
        {/if}
        {#if spaSuccess}
          <div class="callout callout-success" role="status">
            <i class="lni lni-check-circle-1" aria-hidden="true"></i>
            <div class="callout-body"><span>{spaSuccess}</span></div>
          </div>
        {/if}

        {#if spaLoading}
          <p class="provider-note">Loading…</p>
        {:else if spaStatus?.connected}
          <p class="provider-note">
            Connected as <strong>{spaStatus.username ?? 'your Spotify account'}</strong>
          </p>
          <p class="provider-note">
            Syncing creates a Spotify Liked Songs playlist and downloads every liked track
            not already in your library. It runs as a background task on the Tasks page.
          </p>
          <div class="provider-actions">
            <button class="btn-accent" disabled={spaPending} onclick={handleSpotifyAudioSyncLikes}>
              {#if spaPending}<span class="spinner"></span>Working{:else}<i class="lni lni-heart" aria-hidden="true"></i>Sync my Liked Songs{/if}
            </button>
            <button class="btn-danger" disabled={spaPending} onclick={handleSpotifyAudioDisconnect}>
              {#if spaPending}<span class="spinner"></span>Disconnecting{:else}<i class="lni lni-plug-1" aria-hidden="true"></i>Disconnect{/if}
            </button>
          </div>
        {:else}
          {#if spaAuthorizing}
            <p class="provider-note">
              Approve in the Spotify tab, then paste the URL it redirects to (it starts with
              <code>http://127.0.0.1:8898/login?code=</code>) here:
            </p>
            <form class="create-row" onsubmit={(e) => { e.preventDefault(); handleSpotifyAudioComplete(); }}>
              <input
                class="input"
                type="text"
                placeholder="http://127.0.0.1:8898/login?code=..."
                bind:value={spaRedirectUrl}
                disabled={spaPending}
              />
              <button type="submit" class="btn-accent" disabled={spaPending || !spaRedirectUrl.trim()}>
                {#if spaPending}<span class="spinner"></span>Connecting{:else}<i class="lni lni-check" aria-hidden="true"></i>Complete connection{/if}
              </button>
            </form>
          {:else}
            <div class="provider-actions">
              <button class="btn-accent" disabled={spaPending} onclick={handleSpotifyAudioConnect}>
                {#if spaPending}<span class="spinner"></span>Opening{:else}<i class="lni lni-spotify" aria-hidden="true"></i>Connect Spotify{/if}
              </button>
            </div>
          {/if}
        {/if}
      </div>
    </section>
  {/if}
</div>

<style>
  .tools-page {
    width: 100%;
    box-sizing: border-box;
    padding: 1.5rem 2rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1.75rem;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    max-width: 68ch;
  }
  h1 {
    font-size: 1.25rem;
    font-weight: 700;
    margin: 0;
  }
  @media (min-width: 768px) {
    h1 {
      font-size: 1.5rem;
    }
  }
  .lede {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
    line-height: 1.55;
  }

  /* ── Tabs ────────────────────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0.35rem;
    padding: 0.3rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    align-self: flex-start;
    flex-wrap: wrap;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.5rem 0.95rem;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease;
  }
  .tab .lni {
    font-size: 15px;
  }
  .tab:hover:not(.active) {
    color: var(--text);
    background: var(--surface-2);
  }
  .tab.active {
    background: var(--surface-2);
    color: var(--text-bright);
  }

  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  /* ── Section labels ──────────────────────────────────────────────────── */
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--muted-2);
    margin: 0;
  }
  .count {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    letter-spacing: 0;
    padding: 0.1rem 0.4rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .section-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  /* ── Buttons ─────────────────────────────────────────────────────────── */
  .btn-accent,
  .btn-ghost,
  .btn-danger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.55rem 1rem;
    border-radius: 8px;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      filter 0.12s ease,
      background 0.12s ease,
      opacity 0.12s ease;
  }
  .btn-accent .lni,
  .btn-ghost .lni,
  .btn-danger .lni {
    font-size: 16px;
  }
  .btn-accent {
    border: none;
    background: var(--accent);
    color: #fff;
  }
  .btn-accent:hover:not(:disabled) {
    filter: brightness(1.08);
  }
  .btn-ghost {
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .btn-danger {
    background: transparent;
    border: 1px solid color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }
  .btn-danger:hover:not(:disabled) {
    background: var(--error-bg);
  }
  .btn-accent:disabled,
  .btn-ghost:disabled,
  .btn-danger:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn-sm {
    padding: 0.4rem 0.8rem;
    font-size: 0.82rem;
  }

  /* ── Inputs ──────────────────────────────────────────────────────────── */
  .field {
    display: flex;
    align-items: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding-left: 12px;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .field:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .field-icon {
    font-size: 17px;
    color: var(--muted-2);
    flex-shrink: 0;
  }
  .field:focus-within .field-icon {
    color: var(--accent);
  }
  .field input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-family: inherit;
    font-size: 0.9rem;
    padding: 0.7rem 0.7rem;
  }
  .field input::placeholder {
    color: var(--muted);
  }
  .input {
    min-width: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text);
    font-family: inherit;
    font-size: 0.9rem;
    padding: 0.6rem 0.75rem;
    outline: none;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .input::placeholder {
    color: var(--muted);
  }
  .input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .input:disabled {
    opacity: 0.6;
  }

  /* ── Create schedule form ────────────────────────────────────────────── */
  .create-panel {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .create-row {
    display: flex;
    gap: 0.6rem;
  }
  .create-row .input {
    flex: 1;
  }
  .seg {
    display: inline-flex;
    padding: 3px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    flex-shrink: 0;
  }
  .seg-btn {
    padding: 0.35rem 0.85rem;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background 0.12s ease,
      color 0.12s ease;
  }
  .seg-btn:hover:not(.active):not(:disabled) {
    color: var(--text);
  }
  .seg-btn.active {
    background: var(--accent);
    color: #fff;
  }
  .seg-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .interval-group {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .interval-group input {
    width: 5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 0.9rem;
    padding: 0.6rem 0.7rem;
    outline: none;
    transition: border-color 0.15s ease;
  }
  .interval-group input:focus {
    border-color: var(--accent);
  }
  .unit {
    font-size: 0.85rem;
    color: var(--muted);
  }
  .field-error {
    margin: 0;
    font-size: 0.85rem;
    color: var(--error);
  }

  /* ── Callouts ────────────────────────────────────────────────────────── */
  .callout {
    display: flex;
    align-items: flex-start;
    gap: 0.7rem;
    padding: 0.85rem 1rem;
    border-radius: 10px;
    border: 1px solid transparent;
    font-size: 0.9rem;
  }
  .callout .lni {
    font-size: 19px;
    flex-shrink: 0;
    line-height: 1.35;
  }
  .callout-body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .callout-body strong {
    font-weight: 600;
    color: var(--text-bright);
  }
  .callout-error {
    background: var(--error-bg);
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
    color: var(--error);
  }
  .callout-success {
    background: color-mix(in srgb, var(--success) 16%, var(--panel));
    border-color: color-mix(in srgb, var(--success) 45%, transparent);
    color: var(--success);
  }
  .callout-info {
    background: var(--accent-muted);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
    color: var(--accent-2);
  }

  /* ── Status pills ────────────────────────────────────────────────────── */
  .pill {
    display: inline-flex;
    align-items: center;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.15rem 0.6rem;
    border-radius: 999px;
    white-space: nowrap;
  }
  .pill-neutral {
    background: var(--surface-2);
    color: var(--muted);
    font-family: var(--font-mono);
    letter-spacing: 0;
  }
  .pill-success {
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
  }
  .pill-muted {
    background: var(--surface-2);
    color: var(--muted);
  }

  /* ── Storage summary ─────────────────────────────────────────────────── */
  .storage-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 2rem;
    padding: 1rem 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .summary-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .summary-label {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted-2);
  }
  .summary-value {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-bright);
  }
  .summary-mono {
    font-family: var(--font-mono);
    font-size: 0.95rem;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  /* ── Artists list ────────────────────────────────────────────────────── */
  .artists-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .artist-row {
    display: grid;
    grid-template-columns: minmax(8rem, 1.2fr) 2fr auto;
    align-items: center;
    gap: 0.85rem;
    padding: 0.55rem 0.7rem;
    border-radius: 8px;
    transition: background 0.1s ease;
  }
  .artist-row:not(.skeleton):hover {
    background: var(--surface);
  }
  .artist-name {
    font-size: 0.88rem;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar-track {
    height: 6px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    width: 100%;
    background: var(--accent);
    border-radius: 999px;
    transform-origin: left;
    transition: transform 0.3s ease;
  }
  .artist-meta {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    justify-content: flex-end;
  }
  .meta-pct {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .meta-size {
    font-family: var(--font-mono);
    font-size: 0.76rem;
    color: var(--muted-2);
    min-width: 5.5rem;
    text-align: right;
  }

  /* ── Schedule list ───────────────────────────────────────────────────── */
  .schedule-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .schedule-panel {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    padding: 1rem 1.1rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
    transition: opacity 0.12s ease;
  }
  .schedule-panel.disabled {
    opacity: 0.6;
  }
  .schedule-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }
  .schedule-info {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }
  .schedule-label {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .schedule-url {
    font-family: var(--font-mono);
    font-size: 0.76rem;
    color: var(--muted-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .schedule-meta {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }
  .schedule-dates {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem 1.25rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .schedule-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  /* ── Provider panels ─────────────────────────────────────────────────── */
  .provider-panel {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.1rem 1.2rem;
    background: var(--surface);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
  }
  .provider-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .provider-title {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .provider-brand {
    font-size: 22px;
    color: var(--text-bright);
  }
  .provider-name {
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-bright);
  }
  .provider-note {
    margin: 0;
    font-size: 0.86rem;
    line-height: 1.55;
    color: var(--muted);
  }
  .provider-note strong {
    color: var(--text-bright);
    font-weight: 600;
  }
  .provider-note code {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    background: var(--surface-2);
    padding: 0.1rem 0.35rem;
    border-radius: 5px;
    color: var(--text);
  }
  .provider-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  /* ── Empty + loading ─────────────────────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.35rem;
    padding: 2.5rem 1rem;
  }
  .empty .lni {
    font-size: 30px;
    color: var(--muted-2);
    margin-bottom: 0.3rem;
  }
  .empty-title {
    margin: 0;
    font-weight: 600;
    color: var(--text);
  }
  .empty-hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .sk {
    height: 0.75rem;
    border-radius: 4px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s ease-in-out infinite;
  }
  .artist-row.skeleton {
    grid-template-columns: minmax(8rem, 1.2fr) 2fr auto;
  }
  .sk-name {
    width: 60%;
  }
  .sk-bar {
    height: 6px;
    width: 100%;
  }
  .sk-meta {
    width: 4rem;
    justify-self: end;
  }
  .schedule-panel.skeleton {
    gap: 0.5rem;
  }
  .schedule-panel.skeleton .sk-name {
    width: 40%;
  }
  .schedule-panel.skeleton .sk-sub {
    width: 25%;
    height: 0.6rem;
  }
  @keyframes sk-pulse {
    50% {
      opacity: 0.45;
    }
  }

  .spinner {
    width: 13px;
    height: 13px;
    border: 2px solid color-mix(in srgb, currentColor 30%, transparent);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sk,
    .spinner,
    .bar-fill {
      animation: none;
      transition: none;
    }
  }

  @media (max-width: 640px) {
    .tools-page {
      padding: 1.25rem 1rem 1.5rem;
    }
    .create-row {
      flex-direction: column;
    }
    .seg {
      align-self: stretch;
    }
    .seg-btn {
      flex: 1;
    }
    .artist-row {
      grid-template-columns: 1fr auto;
      grid-template-areas:
        "name meta"
        "bar bar";
      row-gap: 0.4rem;
    }
    .artist-name {
      grid-area: name;
    }
    .bar-track {
      grid-area: bar;
    }
    .artist-meta {
      grid-area: meta;
    }
  }
</style>
