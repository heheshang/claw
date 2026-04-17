<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { useLogger } from '../composables/useLogger'

const router = useRouter()
const { initLogger, info, warn } = useLogger()

const username = ref('')
const password = ref('')
const error = ref('')
const loading = ref(false)

onMounted(async () => {
  await initLogger()
  info('Login page loaded')
})

interface AuthResponse {
  token: string
  user: {
    id: number
    username: string
  }
}

async function handleLogin() {
  error.value = ''
  loading.value = true

  try {
    info('Login attempt', { username: username.value })

    const response = await invoke<AuthResponse>('login', {
      request: {
        username: username.value,
        password: password.value
      }
    })

    localStorage.setItem('token', response.token)
    localStorage.setItem('user', JSON.stringify(response.user))

    info('Login success', { userId: response.user.id })
    router.push('/home')
  } catch (e: any) {
    const errorMsg = e.toString()
    warn('Login failed', { username: username.value, error: errorMsg })
    error.value = errorMsg
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="auth-container">
    <div class="auth-card">
      <h1>登录</h1>

      <form @submit.prevent="handleLogin">
        <div class="form-group">
          <label for="username">用户名</label>
          <input
            id="username"
            v-model="username"
            type="text"
            placeholder="请输入用户名"
            required
          />
        </div>

        <div class="form-group">
          <label for="password">密码</label>
          <input
            id="password"
            v-model="password"
            type="password"
            placeholder="请输入密码"
            required
          />
        </div>

        <p v-if="error" class="error">{{ error }}</p>

        <button type="submit" :disabled="loading">
          {{ loading ? '登录中...' : '登录' }}
        </button>
      </form>

      <p class="switch-auth">
        还没有账号？ <router-link to="/register">立即注册</router-link>
      </p>
    </div>
  </div>
</template>

<style scoped>
.auth-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.auth-card {
  background: white;
  padding: 2rem;
  border-radius: 12px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
  width: 100%;
  max-width: 400px;
}

h1 {
  text-align: center;
  margin-bottom: 1.5rem;
  color: #333;
}

.form-group {
  margin-bottom: 1rem;
}

label {
  display: block;
  margin-bottom: 0.5rem;
  color: #555;
  font-weight: 500;
}

input {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 1rem;
  box-sizing: border-box;
}

input:focus {
  outline: none;
  border-color: #667eea;
}

button {
  width: 100%;
  padding: 0.75rem;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 1rem;
  font-weight: 500;
  cursor: pointer;
  margin-top: 1rem;
}

button:hover:not(:disabled) {
  opacity: 0.9;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.error {
  color: #e74c3c;
  font-size: 0.875rem;
  margin-top: 0.5rem;
}

.switch-auth {
  text-align: center;
  margin-top: 1rem;
  color: #666;
}

.switch-auth a {
  color: #667eea;
  text-decoration: none;
}

.switch-auth a:hover {
  text-decoration: underline;
}
</style>
