<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useLogger } from '../composables/useLogger'

const router = useRouter()
const { initLogger, info } = useLogger()

const user = ref<{ id: number; username: string; nickname?: string } | null>(null)

onMounted(async () => {
  await initLogger()
  const storedUser = localStorage.getItem('user')
  if (storedUser) {
    user.value = JSON.parse(storedUser)
    info('Home page loaded', { userId: user.value?.id })
  }
})

function goToChat() {
  router.push('/chat')
}

function goToProfile() {
  router.push('/profile')
}

function handleLogout() {
  info('User logout', { userId: user.value?.id })
  localStorage.removeItem('token')
  localStorage.removeItem('user')
  localStorage.removeItem('permission')
  router.push('/login')
}
</script>

<template>
  <div class="home-container">
    <div class="hero-section">
      <div class="hero-icon">⚡</div>
      <h1>欢迎使用 Claw</h1>
      <p>智能编程助手，让开发更高效</p>
    </div>

    <div class="actions-grid">
      <button class="action-card primary" @click="goToChat">
        <span class="action-icon">💬</span>
        <span class="action-title">开始对话</span>
        <span class="action-desc">与 AI 助手交流</span>
      </button>

      <button class="action-card" @click="goToProfile">
        <span class="action-icon">⚙️</span>
        <span class="action-title">个人设置</span>
        <span class="action-desc">管理账户信息</span>
      </button>
    </div>

    <div v-if="user" class="user-card">
      <div class="user-avatar">{{ user.username?.charAt(0).toUpperCase() }}</div>
      <div class="user-info">
        <span class="username">{{ user.nickname || user.username }}</span>
        <span class="user-id">ID: {{ user.id }}</span>
      </div>
      <button class="logout-btn" @click="handleLogout">退出</button>
    </div>
  </div>
</template>

<style scoped>
.home-container {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  background: #0f0f1a;
}

.hero-section {
  text-align: center;
  margin-bottom: 48px;
}

.hero-icon {
  font-size: 64px;
  margin-bottom: 16px;
}

.hero-section h1 {
  font-size: 32px;
  font-weight: 700;
  color: #fff;
  margin: 0 0 12px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.hero-section p {
  font-size: 16px;
  color: rgba(255, 255, 255, 0.6);
  margin: 0;
}

.actions-grid {
  display: flex;
  gap: 16px;
  margin-bottom: 48px;
}

.action-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px 48px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 16px;
  cursor: pointer;
  transition: all 0.2s;
  min-width: 180px;
}

.action-card:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: translateY(-2px);
}

.action-card.primary {
  background: linear-gradient(135deg, rgba(102, 126, 234, 0.2) 0%, rgba(118, 75, 162, 0.2) 100%);
  border-color: rgba(102, 126, 234, 0.3);
}

.action-card.primary:hover {
  background: linear-gradient(135deg, rgba(102, 126, 234, 0.3) 0%, rgba(118, 75, 162, 0.3) 100%);
}

.action-icon {
  font-size: 32px;
  margin-bottom: 12px;
}

.action-title {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
  margin-bottom: 4px;
}

.action-desc {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
}

.user-card {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
}

.user-avatar {
  width: 44px;
  height: 44px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  font-weight: 600;
  color: #fff;
}

.user-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.username {
  font-size: 15px;
  font-weight: 600;
  color: #fff;
}

.user-id {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.logout-btn {
  padding: 8px 16px;
  background: rgba(255, 100, 100, 0.15);
  border: 1px solid rgba(255, 100, 100, 0.3);
  border-radius: 8px;
  color: #ff6b6b;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
  margin-left: 16px;
}

.logout-btn:hover {
  background: rgba(255, 100, 100, 0.25);
}
</style>
