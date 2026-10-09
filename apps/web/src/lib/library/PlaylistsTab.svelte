<script lang="ts">
  import { lib } from './store.svelte';
  import CardActions from './CardActions.svelte';
  import LibraryOptions from './LibraryOptions.svelte';
  import PixelCover from '../PixelCover.svelte';
  import { runNavigation } from '../navigation-motion';
  import type { PlaylistTrackDto } from '../types';

  function fmtDuration(secs: number | null): string {
    if (secs == null) return '—';
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  function remoteCover(src: string | null | undefined): string | null {
    return src && /^https?:\/\//.test(src) ? src : null;
  }

  /** Total running time, e.g. 48:31 or 1:12:05. */
  function totalDuration(tracks: PlaylistTrackDto[]): string | null {
    const secs = tracks.reduce((sum, t) => sum + (t.duration ?? 0), 0);
    if (secs <= 0) return null;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = String(Math.floor(secs % 60)).padStart(2, '0');
    return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
  }

  function playlistMeta(source: string): string {
    const n = lib.drillPlaylistTracks.length;
    return [source, `${n} track${n !== 1 ? 's' : ''}`, totalDuration(lib.drillPlaylistTracks)].filter(Boolean).join(' · ');
  }
</script>

<!-- ── PLAYLIST DETAIL ──────────────────────────────────────────────────────── -->
{#if lib.drillPlaylistId != null}
  {#if lib.drillPlaylistTracksLoading}
    <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading tracks…</p>
  {:else if lib.drillPlaylistTracksError}
    <div class="callout callout-error" role="alert">
      <i class="pxi pxi-square-alert" aria-hidden="true"></i>
      <div class="callout-body"><strong>Couldn't load tracks</strong><span>{lib.drillPlaylistTracksError}</span></div>
    </div>
  {:else}
    {@const playlist = lib.drillPlaylist}
    {#if playlist}
      <header class="detail-hero">
        <div class="cover-wrap detail-cover">
          <PixelCover src={remoteCover(playlist.cover)} seed="playlist:{playlist.id}" alt={playlist.name} loading="eager" />
        </div>
        <div class="detail-info">
          <h2>{playlist.name}</h2>
          <p class="detail-meta">{playlistMeta(playlist.source)}</p>
          <div class="detail-actions">
            {#if playlist.source_url}
              <a class="btn-header" href={playlist.source_url} target="_blank" rel="noopener noreferrer">
                Open source <i class="pxi pxi-external-link" aria-hidden="true"></i>
              </a>
            {/if}
            <CardActions inline title={playlist.name} actions={[
              { label: 'Delete playlist', danger: true, onSelect: () => lib.handleDeletePlaylist(playlist.id) },
            ]} />
          </div>
        </div>
      </header>
    {:else}
      <p class="status">Loading playlist…</p>
    {/if}

    {#if lib.drillPlaylistTracks.length === 0}
      <div class="empty">
        <i class="pxi pxi-bulletlist" aria-hidden="true"></i>
        <p class="empty-title">No tracks in this playlist.</p>
      </div>
    {:else}
      <div class="table-wrap">
        <table>
          <thead>
            <tr><th>#</th><th>Title</th><th>Artists</th><th>Album</th><th>Genre</th><th>Duration</th></tr>
          </thead>
          <tbody>
            {#each lib.drillPlaylistTracks as t, i (t.id)}
              <tr>
                <td class="mono">{String(i + 1).padStart(2, '0')}</td>
                <td class="pl-title">{t.title}</td>
                <td class="muted">{t.artists.map(a => a.name).join(', ') || '—'}</td>
                <td class="muted">{t.album?.title ?? '—'}</td>
                <td class="muted">{t.genre ?? '—'}</td>
                <td class="mono">{fmtDuration(t.duration)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}

<!-- ── PLAYLISTS GRID ─────────────────────────────────────────────────────── -->
{:else if lib.playlistsLoading}
  <p class="status"><i class="pxi pxi-loader pxi-spin" aria-hidden="true"></i> Loading…</p>
{:else if lib.playlistsError}
  <div class="callout callout-error" role="alert">
    <i class="pxi pxi-square-alert" aria-hidden="true"></i>
    <div class="callout-body"><strong>Couldn't load playlists</strong><span>{lib.playlistsError}</span></div>
  </div>
{:else}
  <LibraryOptions>
    {#snippet search()}
      <i class="pxi pxi-search" aria-hidden="true"></i>
      <input class="search" aria-label="Search playlists" placeholder="Search playlists…" bind:value={lib.playlistSearch} />
    {/snippet}
    {#snippet children()}
      <div class="opt-row">
        <button class="btn-header" onclick={lib.handleRefresh} disabled={lib.refreshing}>
          <i class="pxi pxi-refresh" class:pxi-spin={lib.refreshing} aria-hidden="true"></i>
          {lib.refreshing ? 'Refreshing…' : 'Refresh playlists'}
        </button>
      </div>
      <span class="count">{lib.filteredPlaylists.length} playlist{lib.filteredPlaylists.length !== 1 ? 's' : ''}</span>
    {/snippet}
  </LibraryOptions>

  {#if lib.filteredPlaylists.length === 0}
    <div class="empty">
      <i class="pxi pxi-bulletlist" aria-hidden="true"></i>
      <p class="empty-title">No playlists found.</p>
    </div>
  {:else}
    <div class="card-grid">
      {#each lib.filteredPlaylists as p (p.id)}
        <div class="card clickable">
          <button class="card-main" onclick={() => runNavigation(() => lib.drillIntoPlaylist(p))}>
            <div class="cover-wrap">
              <PixelCover src={remoteCover(p.cover)} seed="playlist:{p.id}" alt={p.name} />
            </div>
            <div class="card-body">
              <div class="card-title" title={p.name}>{p.name}</div>
              <div class="card-meta source">{p.source}</div>
            </div>
          </button>
          <CardActions title={p.name} actions={[
            { label: 'Delete playlist', danger: true, onSelect: () => lib.handleDeletePlaylist(p.id) },
          ]} />
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .card-main {
    display: block;
    width: 100%;
    min-width: 0;
    padding: 0;
    border: none;
    border-radius: var(--radius-control);
    background: none;
    color: inherit;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
  }
  .source { text-transform: uppercase; letter-spacing: 0.06em; }

  /* Detail hero: no card chrome; the cover carries the hairline. */
  .detail-hero { display: flex; align-items: flex-end; gap: 20px; margin-bottom: 32px; }
  .detail-cover { width: 120px; height: 120px; flex: 0 0 auto; }
  .detail-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .detail-info h2 { margin: 0; font-size: 28px; line-height: 1.15; letter-spacing: -0.03em; overflow-wrap: anywhere; }
  .detail-meta { margin: 0; font-family: var(--font-mono); font-size: 12px; font-variant-numeric: tabular-nums; color: var(--muted-2); }
  .detail-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 12px; }
  .detail-actions a { text-decoration: none; }

  .pl-title { font-weight: 500; color: var(--text-bright); }
  .status { display: flex; align-items: center; justify-content: center; gap: 8px; }
  .status .pxi { font-size: 16px; }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .detail-hero { align-items: flex-start; gap: 16px; margin-bottom: 24px; }
    .detail-info h2 { font-size: 24px; }
    table { min-width: 600px; }
  }
</style>
