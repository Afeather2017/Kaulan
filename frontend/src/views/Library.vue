<template>
  <div class="library">
    <h2>Music Library</h2>
    
    <div v-if="loading" class="loading">Loading songs...</div>
    
    <div v-else class="songs-grid">
      <div v-for="song in songs" :key="song.id" class="song-card">
        <h3>{{ song.title }}</h3>
        <p>{{ song.artist }}</p>
        <span class="duration">{{ formatDuration(song.duration) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

interface Song {
  id: number
  title: string
  artist: string
  duration: number
}

const songs = ref<Song[]>([])
const loading = ref(true)

const formatDuration = (seconds: number): string => {
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`
}

onMounted(async () => {
  try {
    const response = await fetch('/api/music/songs')
    songs.value = await response.json()
  } catch (error) {
    console.error('Failed to fetch songs:', error)
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
.songs-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 1rem;
  margin-top: 1rem;
}

.song-card {
  padding: 1rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  cursor: pointer;
  transition: transform 0.2s;
}

.song-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 8px rgba(0,0,0,0.1);
}

.song-card h3 {
  margin: 0 0 0.5rem 0;
}

.song-card p {
  margin: 0 0 0.5rem 0;
  color: #666;
}

.duration {
  font-size: 0.9rem;
  color: #999;
}

.loading {
  text-align: center;
  padding: 2rem;
  color: #666;
}
</style>