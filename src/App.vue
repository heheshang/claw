<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()
const token = localStorage.getItem('token')
const isAuthenticated = computed(() => !!token)
const storedUser = computed(() => {
  try {
    return JSON.parse(localStorage.getItem('user') || '{}')
  } catch {
    return {}
  }
})

const navItems = [
  { path: '/chat', label: 'AI 助手', icon: '💬' },
  { path: '/home', label: '首页', icon: '🏠' },
  { path: '/profile', label: '设置', icon: '⚙️' }
]

const handleLogout = () => {
  localStorage.removeItem('token')
  localStorage.removeItem('user')
  localStorage.removeItem('permission')
  router.push('/login')
}
</script>

<template>
  <div class="app-layout">
    <aside v-if="isAuthenticated" class="sidebar">
      <div class="sidebar-header">
        <div class="logo">
          <span class="logo-icon">⚡</span>
          <span class="logo-text">Claw</span>
        </div>
      </div>

      <nav class="sidebar-nav">
        <router-link
          v-for="item in navItems"
          :key="item.path"
          :to="item.path"
          class="nav-item"
          :class="{ active: route.path.startsWith(item.path) }"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          <span class="nav-label">{{ item.label }}</span>
        </router-link>
      </nav>

      <div class="sidebar-footer">
        <div class="user-hint">登录为 {{ storedUser.username || '用户' }}</div>
        <button @click="handleLogout" class="logout-btn">
          <span>🚪</span> 退出
        </button>
      </div>
    </aside>

    <main class="main-content">
      <router-view />
    </main>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  width: 100%;
  height: 100%;
}

.app-layout {
  display: flex;
  width: 100%;
  height: 100%;
  background: #0f0f1a;
}

.sidebar {
  width: 240px;
  height: 100%;
  background: linear-gradient(180deg, #1a1a2e 0%, #16162a 100%);
  color: #fff;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  border-right: 1px solid rgba(255, 255, 255, 0.06);
}

.sidebar-header {
  padding: 24px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
}

.logo-icon {
  font-size: 24px;
}

.logo-text {
  font-size: 22px;
  font-weight: 700;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: -0.5px;
}

.sidebar-nav {
  flex: 1;
  padding: 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 10px;
  color: rgba(255, 255, 255, 0.6);
  text-decoration: none;
  transition: all 0.2s ease;
  font-size: 14px;
  font-weight: 500;
}

.nav-item:hover {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.9);
}

.nav-item.active {
  background: linear-gradient(135deg, rgba(102, 126, 234, 0.25) 0%, rgba(118, 75, 162, 0.25) 100%);
  color: #fff;
  border: 1px solid rgba(102, 126, 234, 0.3);
}

.nav-icon {
  font-size: 18px;
}

.nav-label {
  font-size: 14px;
}

.sidebar-footer {
  padding: 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.user-hint {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  margin-bottom: 12px;
  padding: 0 4px;
}

.logout-btn {
  width: 100%;
  padding: 10px 16px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  gap: 8px;
}

.logout-btn:hover {
  background: rgba(255, 100, 100, 0.15);
  border-color: rgba(255, 100, 100, 0.3);
  color: #ff6b6b;
}

.main-content {
  flex: 1;
  height: 100%;
  overflow: hidden;
}
</style>
