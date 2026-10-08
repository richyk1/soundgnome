<script lang="ts">
  import { onMount } from 'svelte';
  import BackfillPanel from '../lib/BackfillPanel.svelte';
  import MissingFiles from '../lib/MissingFiles.svelte';
  import { runNavigation } from '../lib/navigation-motion';
  import { getStorageStats, getSyncSchedules, createSyncSchedule, updateSyncSchedule, deleteSyncSchedule, triggerSyncSchedule, getSoundcloudStatus, connectSoundcloud, disconnectSoundcloud,
    getSpotifyAudioStatus, connectSpotifyAudio, completeSpotifyAudio, disconnectSpotifyAudio, downloadUrl, embedArtwork, backfillFingerprints } from '../lib/api';
  import type { StorageStatsDto, SyncScheduleDto, SoundcloudStatusDto, SpotifyAudioStatusDto } from '../lib/api';
  import { getLastfmStatus, setLastfmCredentials, lastfmLogin, lastfmComplete, disconnectLastfm, type LastfmStatusDto } from '../lib/api';
  import { isScrobbleEnabled, setScrobbleEnabled, refreshStatus as refreshScrobbler } from '../lib/scrobbler';

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
    loadLastfm();
  });

  function switchTab(tab: Tab) {
    if (tab === activeTab) return;
    void runNavigation(() => { activeTab = tab; });
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

  // ── Last.fm ───────────────────────────────────────────────────────────────────
  let lfmStatus: LastfmStatusDto | null = $state(null);
  let lfmLoading = $state(true);
  let lfmPending = $state(false);
  let lfmError: string | null = $state(null);
  let lfmSuccess: string | null = $state(null);
  let lfmKey = $state('');
  let lfmSecret = $state('');
  let lfmToken: string | null = $state(null);
  let lfmAuthUrl: string | null = $state(null);
  const LFM_PENDING = 'soundgnome:lastfm:pending';
  function savePending(url: string, token: string) {
    lfmAuthUrl = url;
    lfmToken = token;
    try {
      localStorage.setItem(LFM_PENDING, JSON.stringify({ url, token }));
    } catch {
      /* storage unavailable */
    }
  }
  function clearPending() {
    lfmAuthUrl = null;
    lfmToken = null;
    try {
      localStorage.removeItem(LFM_PENDING);
    } catch {
      /* storage unavailable */
    }
  }
  let scrobbleOn = $state(isScrobbleEnabled());

  async function loadLastfm() {
    lfmLoading = true;
    lfmError = null;
    try {
      lfmStatus = await getLastfmStatus();
      if (!lfmStatus.connected) {
        // Restore a pending authorization so a reload doesn't lose the finish step.
        try {
          const raw = localStorage.getItem(LFM_PENDING);
          if (raw) {
            const p = JSON.parse(raw);
            lfmAuthUrl = typeof p?.url === 'string' ? p.url : null;
            lfmToken = typeof p?.token === 'string' ? p.token : null;
          }
        } catch {
          /* ignore malformed pending */
        }
      }
    } catch (e: unknown) {
      lfmError = e instanceof Error ? e.message : String(e);
    } finally {
      lfmLoading = false;
    }
  }

  async function handleLastfmCredentials(e: Event) {
    e.preventDefault();
    if (lfmPending || !lfmKey.trim() || !lfmSecret.trim()) return;
    lfmPending = true;
    lfmError = null;
    lfmSuccess = null;
    try {
      lfmStatus = await setLastfmCredentials(lfmKey.trim(), lfmSecret.trim());
      lfmSecret = '';
      lfmSuccess = 'API credentials saved. Now connect your account.';
    } catch (e: unknown) {
      lfmError = e instanceof Error ? e.message : String(e);
    } finally {
      lfmPending = false;
    }
  }

  async function handleLastfmConnect() {
    if (lfmPending) return;
    lfmPending = true;
    lfmError = null;
    lfmSuccess = null;
    try {
      const { url, token } = await lastfmLogin();
      savePending(url, token);
      window.open(url, '_blank', 'noopener');
    } catch (e: unknown) {
      lfmError = e instanceof Error ? e.message : String(e);
    } finally {
      lfmPending = false;
    }
  }

  async function handleLastfmComplete() {
    if (lfmPending || !lfmToken) return;
    lfmPending = true;
    lfmError = null;
    try {
      lfmStatus = await lastfmComplete(lfmToken);
      clearPending();
      refreshScrobbler();
      lfmSuccess = `Last.fm connected as ${lfmStatus.username ?? 'your account'}.`;
    } catch (e: unknown) {
      lfmError = e instanceof Error ? e.message : String(e);
    } finally {
      lfmPending = false;
    }
  }

  async function handleLastfmDisconnect() {
    lfmPending = true;
    lfmError = null;
    lfmSuccess = null;
    try {
      lfmStatus = await disconnectLastfm();
      refreshScrobbler();
      clearPending();
    } catch (e: unknown) {
      lfmError = e instanceof Error ? e.message : String(e);
    } finally {
      lfmPending = false;
    }
  }

  const headerMeta = $derived.by(() => {
    const parts: string[] = [];
    if (!syncLoading) parts.push(`${schedules.length} ${schedules.length === 1 ? 'schedule' : 'schedules'}`);
    if (!scLoading && !spaLoading && !lfmLoading) {
      const connected = [scStatus, spaStatus, lfmStatus].filter((st) => st?.connected).length;
      parts.push(`${connected}/3 accounts connected`);
    }
    if (stats) parts.push(stats.total_formatted);
    return parts.join(' · ');
  });

  function toggleScrobble() {
    scrobbleOn = !scrobbleOn;
    setScrobbleEnabled(scrobbleOn);
  }
</script>

<div class="tools-page">
  <header class="page-header">
    <h1>Tools</h1>
    {#if headerMeta}<p class="header-meta mono">{headerMeta}</p>{/if}
    <p class="lede">
      Connect external accounts, schedule automatic playlist syncs, and keep an eye on how your
      library uses disk.
    </p>
  </header>

  <div class="tabs" role="tablist">
    <button class="tab" class:active={activeTab === 'sync'} onclick={() => switchTab('sync')}>
      <i class="pxi pxi-refresh" aria-hidden="true"></i>Sync
    </button>
    <button class="tab" class:active={activeTab === 'storage'} onclick={() => switchTab('storage')}>
      <i class="pxi pxi-database" aria-hidden="true"></i>Storage
    </button>
    <button class="tab" class:active={activeTab === 'providers'} onclick={() => switchTab('providers')}>
      <i class="pxi pxi-plug" aria-hidden="true"></i>Providers
    </button>
    <button class="tab" class:active={activeTab === 'artwork'} onclick={() => switchTab('artwork')}>
      <i class="pxi pxi-image" aria-hidden="true"></i>Artwork
    </button>
    <button class="tab" class:active={activeTab === 'fingerprints'} onclick={() => switchTab('fingerprints')}>
      <i class="pxi pxi-audio-waveform" aria-hidden="true"></i>Fingerprints
    </button>
    <button class="tab" class:active={activeTab === 'missing'} onclick={() => switchTab('missing')}>
      <i class="pxi pxi-unlink" aria-hidden="true"></i>Missing files
    </button>
  </div>

  <!-- ── Artwork tab ─────────────────────────────────────────────────────────── -->
  {#if activeTab === 'artwork'}
    <section class="tab-content">
      <BackfillPanel
        title="Embed artwork"
        description="Embed cover art into every library file in place so artwork travels with the audio and shows offline. Missing covers are resolved from each track's references (Spotify oEmbed or YouTube thumbnail). Audio is never re-downloaded or moved."
        icon="image"
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
        icon="audio-waveform"
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
    <section class="tab-content narrow">
      <div class="section-head">
        <h2>Library storage</h2>
        <div class="section-actions">
          <button class="btn-ghost btn-sm" onclick={loadStorage} disabled={storageLoading}>
            {#if storageLoading}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>{:else}<i class="pxi pxi-refresh" aria-hidden="true"></i>{/if}Refresh
          </button>
        </div>
      </div>

      {#if storageError}
        <div class="callout callout-error" role="alert">
          <i class="pxi pxi-square-alert" aria-hidden="true"></i>
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
        <dl class="storage-summary">
          <div class="summary-item">
            <dt>Library size</dt>
            <dd class="summary-value mono">{stats.total_formatted}</dd>
          </div>
          <div class="summary-item">
            <dt>Total bytes</dt>
            <dd class="mono">{stats.total_bytes.toLocaleString()}</dd>
          </div>
          <div class="summary-item">
            <dt>Artists</dt>
            <dd class="mono">{stats.artists.length}</dd>
          </div>
        </dl>

        {#if stats.artists.length === 0}
          <div class="empty">
            <i class="pxi pxi-database" aria-hidden="true"></i>
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
                <span class="artist-meta mono">
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
    <section class="tab-content narrow">
      <div class="section-head">
        <h2>Add a schedule</h2>
      </div>
      <p class="lede">Define playlists to synchronize automatically using intervals or cron expressions.</p>

      <form class="create-panel" onsubmit={handleCreate}>
        <div class="field">
          <i class="pxi pxi-download field-icon" aria-hidden="true"></i>
          <input
            class="field-input"
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
              <input class="mono" type="number" min="0.25" step="0.25" bind:value={newIntervalHours} disabled={creating} />
              <span class="unit">hours</span>
            </div>
            <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
              {#if creating}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Adding{:else}<i class="pxi pxi-calendar" aria-hidden="true"></i>Add{/if}
            </button>
          </div>
        {:else}
          <div class="create-row">
            <input class="input" type="text" placeholder="Cron expression (e.g. '0 12 * * *' for daily at noon)" bind:value={newCronExpression} disabled={creating} />
            <button type="submit" class="btn-accent" disabled={creating || !newUrl.trim()}>
              {#if creating}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Adding{:else}<i class="pxi pxi-calendar" aria-hidden="true"></i>Add{/if}
            </button>
          </div>
        {/if}

        {#if createError}
          <p class="field-error">{createError}</p>
        {/if}
      </form>

      {#if triggerMsg}
        <div class="callout callout-info" role="status">
          <i class="pxi pxi-refresh" aria-hidden="true"></i>
          <div class="callout-body"><span>{triggerMsg}</span></div>
        </div>
      {/if}
      {#if syncError}
        <div class="callout callout-error" role="alert">
          <i class="pxi pxi-square-alert" aria-hidden="true"></i>
          <div class="callout-body"><span>{syncError}</span></div>
        </div>
      {/if}

      <div class="section-head">
        <h2>Schedules{#if !syncLoading}<span class="h-count mono">{schedules.length}</span>{/if}</h2>
      </div>

      {#if syncLoading}
        <ul class="schedule-list" aria-hidden="true">
          {#each { length: 3 } as _}
            <li class="schedule-row skeleton">
              <span class="sk sk-name"></span>
              <span class="sk sk-sub"></span>
            </li>
          {/each}
        </ul>
      {:else if schedules.length === 0}
        <div class="empty">
          <i class="pxi pxi-calendar" aria-hidden="true"></i>
          <p class="empty-title">No schedules yet</p>
          <p class="empty-hint">Add a playlist above to sync it automatically.</p>
        </div>
      {:else}
        <ul class="schedule-list">
          {#each schedules as schedule (schedule.id)}
            <li class="schedule-row" class:paused={!schedule.enabled}>
              <div class="schedule-top">
                <div class="schedule-info">
                  <span class="schedule-label">{schedule.label ?? schedule.playlist_url}</span>
                  {#if schedule.label}
                    <span class="schedule-url mono">{schedule.playlist_url}</span>
                  {/if}
                </div>
                <span class="tag" class:tag-success={schedule.enabled}>
                  {schedule.enabled ? 'Active' : 'Paused'}
                </span>
              </div>
              <dl class="schedule-data">
                <div><dt>Interval</dt><dd class="mono">{formatSchedule(schedule)}</dd></div>
                <div><dt>Next run</dt><dd class="mono">{formatDate(schedule.next_run)}</dd></div>
                <div><dt>Last run</dt><dd class="mono">{formatDate(schedule.last_run)}</dd></div>
              </dl>
              <div class="schedule-actions">
                <button class="btn-ghost btn-sm" onclick={() => toggleEnabled(schedule)}>
                  {#if schedule.enabled}<i class="pxi pxi-pause" aria-hidden="true"></i>Pause{:else}<i class="pxi pxi-play" aria-hidden="true"></i>Resume{/if}
                </button>
                <button class="btn-accent btn-sm" disabled={triggeringId === schedule.id} onclick={() => handleTrigger(schedule.id)}>
                  {#if triggeringId === schedule.id}
                    <i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Syncing
                  {:else}
                    <i class="pxi pxi-refresh" aria-hidden="true"></i>Sync now
                  {/if}
                </button>
                <button class="btn-danger btn-sm" onclick={() => handleDelete(schedule.id)}>
                  <i class="pxi pxi-trash" aria-hidden="true"></i>Delete
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
    <section class="tab-content narrow">
      <div class="section-head">
        <h2>Connected accounts</h2>
      </div>
      <p class="lede">
        Connect external accounts so downloads and metadata can use your own access.
      </p>

      <div class="provider-panel">
        <div class="provider-head">
          <h3 class="provider-name">SoundCloud</h3>
          {#if !scLoading}
            <span class="tag" class:tag-success={scStatus?.connected}>
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
            <i class="pxi pxi-square-alert" aria-hidden="true"></i>
            <div class="callout-body"><span>{scError}</span></div>
          </div>
        {/if}
        {#if scSuccess}
          <div class="callout callout-success" role="status">
            <i class="pxi pxi-check" aria-hidden="true"></i>
            <div class="callout-body"><span>{scSuccess}</span></div>
          </div>
        {/if}

        {#if scLoading}
          <p class="provider-note provider-loading"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Loading…</p>
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
              {#if scPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Working{:else}<i class="pxi pxi-heart" aria-hidden="true"></i>Sync my likes{/if}
            </button>
            <button class="btn-danger" disabled={scPending} onclick={handleDisconnect}>
              {#if scPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Disconnecting{:else}<i class="pxi pxi-unlink" aria-hidden="true"></i>Disconnect{/if}
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
              {#if scPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Connecting{:else}<i class="pxi pxi-link" aria-hidden="true"></i>Connect{/if}
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
          <h3 class="provider-name">Spotify</h3>
          {#if !spaLoading}
            <span class="tag" class:tag-success={spaStatus?.connected}>
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
            <i class="pxi pxi-square-alert" aria-hidden="true"></i>
            <div class="callout-body"><span>{spaError}</span></div>
          </div>
        {/if}
        {#if spaSuccess}
          <div class="callout callout-success" role="status">
            <i class="pxi pxi-check" aria-hidden="true"></i>
            <div class="callout-body"><span>{spaSuccess}</span></div>
          </div>
        {/if}

        {#if spaLoading}
          <p class="provider-note provider-loading"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Loading…</p>
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
              {#if spaPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Working{:else}<i class="pxi pxi-heart" aria-hidden="true"></i>Sync my Liked Songs{/if}
            </button>
            <button class="btn-danger" disabled={spaPending} onclick={handleSpotifyAudioDisconnect}>
              {#if spaPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Disconnecting{:else}<i class="pxi pxi-unlink" aria-hidden="true"></i>Disconnect{/if}
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
                {#if spaPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Connecting{:else}<i class="pxi pxi-check" aria-hidden="true"></i>Complete connection{/if}
              </button>
            </form>
          {:else}
            <div class="provider-actions">
              <button class="btn-accent" disabled={spaPending} onclick={handleSpotifyAudioConnect}>
                {#if spaPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Opening{:else}<i class="pxi pxi-link" aria-hidden="true"></i>Connect Spotify{/if}
              </button>
            </div>
          {/if}
        {/if}
      </div>

      <div class="provider-panel">
        <div class="provider-head">
          <h3 class="provider-name">Last.fm</h3>
          {#if !lfmLoading}
            <span class="tag" class:tag-success={lfmStatus?.connected}>
              {lfmStatus?.connected ? 'Connected' : 'Not connected'}
            </span>
          {/if}
        </div>

        <p class="provider-note">
          Scrobble what you play here to your Last.fm profile. Now-playing and completed plays are
          reported automatically (a play counts after half the track, or 4 minutes).
        </p>

        {#if lfmError}
          <div class="callout callout-error" role="alert">
            <i class="pxi pxi-square-alert" aria-hidden="true"></i>
            <div class="callout-body"><span>{lfmError}</span></div>
          </div>
        {/if}
        {#if lfmSuccess}
          <div class="callout callout-success" role="status">
            <i class="pxi pxi-check" aria-hidden="true"></i>
            <div class="callout-body"><span>{lfmSuccess}</span></div>
          </div>
        {/if}

        {#if lfmLoading}
          <p class="provider-note provider-loading"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Loading…</p>
        {:else if lfmStatus?.connected}
          <p class="provider-note">
            Connected as <strong>{lfmStatus.username ?? 'your account'}</strong>
          </p>
          <label class="scrobble-toggle">
            <input type="checkbox" checked={scrobbleOn} onchange={toggleScrobble} />
            <span>Scrobble what I play</span>
          </label>
          <div class="provider-actions">
            <button class="btn-danger" disabled={lfmPending} onclick={handleLastfmDisconnect}>
              {#if lfmPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Disconnecting{:else}<i class="pxi pxi-unlink" aria-hidden="true"></i>Disconnect{/if}
            </button>
          </div>
        {:else if lfmStatus?.configured}
          {#if lfmToken}
            <p class="provider-note">
              A Last.fm tab should have opened. If it didn't,
              {#if lfmAuthUrl}<a href={lfmAuthUrl} target="_blank" rel="noopener noreferrer">open the authorization page</a>{:else}re-open it{/if}.
              Click <strong>Allow access</strong> there, then finish here.
            </p>
            <div class="provider-actions">
              <button class="btn-accent" disabled={lfmPending} onclick={handleLastfmComplete}>
                {#if lfmPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Finishing{:else}<i class="pxi pxi-check" aria-hidden="true"></i>I've approved, finish{/if}
              </button>
              <button class="btn-ghost" disabled={lfmPending} onclick={clearPending}>Cancel</button>
            </div>
          {:else}
            <div class="provider-actions">
              <button class="btn-accent" disabled={lfmPending} onclick={handleLastfmConnect}>
                {#if lfmPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Opening{:else}<i class="pxi pxi-link" aria-hidden="true"></i>Connect Last.fm{/if}
              </button>
            </div>
          {/if}
        {:else}
          <form class="create-row" onsubmit={handleLastfmCredentials}>
            <input class="input" type="text" placeholder="API key" bind:value={lfmKey} disabled={lfmPending} autocomplete="off" spellcheck="false" />
            <input class="input" type="password" placeholder="Shared secret" bind:value={lfmSecret} disabled={lfmPending} autocomplete="off" spellcheck="false" />
            <button type="submit" class="btn-accent" disabled={lfmPending || !lfmKey.trim() || !lfmSecret.trim()}>
              {#if lfmPending}<i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i>Saving{:else}<i class="pxi pxi-link" aria-hidden="true"></i>Save{/if}
            </button>
          </form>
          <p class="provider-note">
            Create a free API account at
            <a href="https://www.last.fm/api/account/create" target="_blank" rel="noopener noreferrer">last.fm/api/account/create</a>
            to get an API key and shared secret.
          </p>
        {/if}
      </div>
    </section>
  {/if}
</div>

<style>
  .tools-page {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-page);
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  /* ── Header ──────────────────────────────────────────────────────────── */
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-start;
    gap: 6px;
    margin: 0;
  }
  .page-header h1 {
    font-size: 32px;
    line-height: 1.05;
  }
  .header-meta {
    margin: 0;
    font-size: 12px;
    color: var(--muted-2);
  }
  .lede {
    margin: 0;
    max-width: 64ch;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .page-header .lede { margin-top: 6px; }

  .tabs { margin-bottom: 0; }

  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .tab-content.narrow { max-width: 880px; }

  /* ── Section heads ───────────────────────────────────────────────────── */
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .section-head + .lede { margin-top: -8px; }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    font-size: 16px;
    line-height: 1.35;
  }
  .h-count {
    font-size: 12px;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--muted-2);
  }
  .tab-content > .section-head:not(:first-child) { margin-top: 12px; }

  /* ── Sync: create form ───────────────────────────────────────────────── */
  .create-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .field { position: relative; }
  .field-icon {
    position: absolute;
    top: 50%;
    left: 12px;
    translate: 0 -50%;
    font-size: 16px;
    color: var(--muted-2);
    pointer-events: none;
    transition: color var(--motion-fast) var(--ease-out);
  }
  .field:focus-within .field-icon { color: var(--accent); }
  .field-input { padding-left: 38px; }
  .create-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .create-row .input { flex: 1; }
  .seg {
    display: inline-flex;
    flex-shrink: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-control);
    overflow: hidden;
  }
  .seg-btn {
    min-height: 36px;
    padding: 0 14px;
    border: 0;
    background: transparent;
    color: var(--muted-2);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .seg-btn + .seg-btn { border-left: 1px solid var(--border-strong); }
  .seg-btn:hover:not(.active):not(:disabled) { background: var(--surface-2); color: var(--text-bright); }
  .seg-btn.active { background: var(--accent-muted); color: var(--text-bright); }
  .seg-btn:disabled { opacity: 0.45; cursor: default; }
  .interval-group {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .interval-group input { width: 6.5rem; }
  .unit {
    font-size: 14px;
    color: var(--muted);
  }
  .field-error {
    margin: 0;
    font-size: 13px;
    color: var(--error);
    overflow-wrap: anywhere;
  }

  /* ── Status tags ─────────────────────────────────────────────────────── */
  .tag {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    min-height: 20px;
    padding: 0 6px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .tag-success {
    border-color: color-mix(in srgb, var(--success) 40%, transparent);
    background: color-mix(in srgb, var(--success) 12%, transparent);
    color: var(--success);
  }

  /* ── Storage ─────────────────────────────────────────────────────────── */
  .storage-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin: 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }
  .summary-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    padding: 14px 16px 14px 0;
  }
  .summary-item + .summary-item {
    padding-left: 16px;
    border-left: 1px solid var(--border-soft);
  }
  dt {
    font-size: 12px;
    color: var(--muted-2);
  }
  dd {
    margin: 0;
    font-size: 14px;
    color: var(--text);
    overflow-wrap: anywhere;
  }
  .summary-value {
    font-size: 22px;
    font-weight: 500;
    line-height: 1.2;
    color: var(--text-bright);
  }

  .artists-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--border);
  }
  .artist-row {
    display: grid;
    grid-template-columns: minmax(8rem, 1.2fr) 2fr auto;
    align-items: center;
    gap: 16px;
    min-height: 44px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-soft);
  }
  .artist-name {
    min-width: 0;
    font-size: 14px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar-track {
    height: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar-fill {
    width: 100%;
    height: 100%;
    background: var(--live);
    transform-origin: left;
    transition: transform var(--motion-normal) var(--ease-out);
  }
  .artist-meta {
    display: flex;
    align-items: baseline;
    justify-content: flex-end;
    gap: 12px;
    font-size: 12px;
  }
  .meta-pct { color: var(--text); }
  .meta-size {
    min-width: 5.5rem;
    text-align: right;
    color: var(--muted-2);
  }

  /* ── Sync: schedule rows ─────────────────────────────────────────────── */
  .schedule-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--border);
  }
  .schedule-row {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 0;
    border-bottom: 1px solid var(--border-soft);
  }
  .schedule-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .schedule-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .schedule-label {
    font-size: 15px;
    font-weight: 500;
    color: var(--text-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .schedule-row.paused .schedule-label { color: var(--muted); }
  .schedule-url {
    font-size: 12px;
    color: var(--muted-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .schedule-data {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 28px;
    margin: 0;
  }
  .schedule-data > div {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .schedule-data dd { font-size: 12px; }
  .schedule-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  /* ── Providers ───────────────────────────────────────────────────────── */
  .provider-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
  }
  .provider-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .provider-name {
    margin: 0;
    font-size: 16px;
    line-height: 1.35;
  }
  .provider-note {
    margin: 0;
    max-width: 68ch;
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }
  .provider-note strong {
    font-weight: 600;
    color: var(--text-bright);
  }
  .provider-note a { color: var(--accent); }
  .provider-note a:hover { color: var(--accent-2); }
  .provider-loading {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .provider-loading .pxi { font-size: 16px; color: var(--muted-2); }
  .provider-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .scrobble-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
    font-size: 14px;
    color: var(--text);
    cursor: pointer;
  }
  .scrobble-toggle input {
    width: auto;
    margin: 0;
    cursor: pointer;
  }

  /* ── Skeletons ───────────────────────────────────────────────────────── */
  .sk {
    height: 10px;
    border-radius: 2px;
    background: var(--surface-2);
    animation: sk-pulse 1.3s steps(4) infinite;
  }
  .sk-name { width: 60%; }
  .sk-bar { width: 100%; height: 4px; }
  .sk-meta { width: 4rem; justify-self: end; }
  .schedule-row.skeleton { gap: 8px; }
  .schedule-row.skeleton .sk-name { width: 40%; }
  .schedule-row.skeleton .sk-sub { width: 25%; height: 8px; }
  @keyframes sk-pulse {
    50% { opacity: 0.45; }
  }

  @media (prefers-reduced-motion: reduce) {
    .sk { animation: none; }
    .bar-fill { transition: none; }
  }

  /* ── Phones ──────────────────────────────────────────────────────────── */
  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .page-header h1 { font-size: 28px; }
    .tab { flex-shrink: 0; }
    .create-panel { padding: 12px; }
    .create-row { flex-direction: column; align-items: stretch; }
    .create-row .input { flex: 0 0 auto; width: 100%; }
    .seg { align-self: stretch; }
    .seg-btn { flex: 1; min-height: 44px; }
    .interval-group input { width: 7rem; }
    .provider-panel { padding: 16px; }
    .provider-head, .schedule-top { flex-wrap: wrap; }
    .schedule-top { gap: 8px; }
    .schedule-info { flex: 1 1 12rem; }
    .schedule-data { flex-direction: column; }
    .schedule-data dt { min-width: 4.5rem; }
    .schedule-actions > button { flex: 1 1 auto; }
    .scrobble-toggle { min-height: 44px; }
    .provider-note { overflow-wrap: anywhere; }
    .storage-summary { grid-template-columns: 1fr 1fr; }
    .summary-item:first-child {
      grid-column: 1 / -1;
      border-bottom: 1px solid var(--border-soft);
    }
    .summary-item:nth-child(2) { padding-left: 0; border-left: 0; }
    .artist-row {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        "name meta"
        "bar bar";
      row-gap: 8px;
    }
    .artist-name { grid-area: name; }
    .bar-track, .sk-bar { grid-area: bar; }
    .artist-meta, .sk-meta { grid-area: meta; }
    .sk-name { grid-area: name; }
  }
</style>
