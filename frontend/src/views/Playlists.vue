<template>
  <div class="playlists">
    <h2>Playlists</h2>
    
    <div v-if="loading" class="loading">Loading playlists...</div>
    
    <div v-else class="playlists-grid">
      <div v-for="playlist in playlists" :key="playlist.id" class="playlist-card">
        <h3>{{ playlist.name }}</h3>
        <p>{{ playlist.songs.length }} songs</p>
        <div class="song-list">
          <div v-for="song in playlist.songs.slice(0, 3)" :key="song.id" class="song-preview">
            {{ song.title }} - {{ song.artist }}
          </div>
          <div v-if="playlist.songs.length > 3" class="more-songs">
            +{{ playlist.songs.length - 3 }} more
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { API_BASE } from '@/utils/api'

interface Song {
  id: number
  title: string
  artist: string
  duration: number
}

interface Playlist {
  id: number
  name: string
  songs: Song[]
}

const playlists = ref<Playlist[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    const response = await fetch(`${API_BASE}/music/playlists`)
    playlists.value = await response.json()
  } catch (error) {
    console.error('Failed to fetch playlists:', error)
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
.playlists-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 1.5rem;
  margin-top: 1rem;
}

.playlist-card {
  padding: 1.5rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  background-color: #f8f9fa;
}

.playlist-card h3 {
  margin: 0 0 0.5rem 0;
}

.playlist-card > p {
  margin: 0 0 1rem 0;
  color: #666;
  font-size: 0.9rem;
}

.song-list {
  font-size: 0.85rem;
}

.song-preview {
  padding: 0.25rem 0;
  border-bottom: 1px solid #eee;
}

.more-songs {
  padding: 0.25rem 0;
  color: #999;
  font-style: italic;
}

.loading {
  text-align: center;
  padding: 2rem;
  color: #666;
}
</style>