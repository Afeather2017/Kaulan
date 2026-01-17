<template>
  <div class="home">
    <h2>Welcome to Aural</h2>
    <p>Your personal music streaming platform</p>
    
    <div class="stats">
      <div class="stat-card">
        <h3>Songs</h3>
        <p>{{ songCount }}</p>
      </div>
      <div class="stat-card">
        <h3>Playlists</h3>
        <p>{{ playlistCount }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const songCount = ref(0)
const playlistCount = ref(0)

onMounted(async () => {
  try {
    const [songsRes, playlistsRes] = await Promise.all([
      fetch('/api/music/songs'),
      fetch('/api/music/playlists')
    ])
    
    const songs = await songsRes.json()
    const playlists = await playlistsRes.json()
    
    songCount.value = songs.length
    playlistCount.value = playlists.length
  } catch (error) {
    console.error('Failed to fetch data:', error)
  }
})
</script>

<style scoped>
.home {
  text-align: center;
}

.stats {
  display: flex;
  justify-content: center;
  gap: 2rem;
  margin-top: 2rem;
}

.stat-card {
  padding: 1rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  min-width: 100px;
}

.stat-card h3 {
  margin: 0 0 0.5rem 0;
  color: #666;
}

.stat-card p {
  margin: 0;
  font-size: 1.5rem;
  font-weight: bold;
}
</style>