<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useLogger } from '../composables/useLogger'

const { initLogger, info, warn } = useLogger()

interface UserProfile {
  id: number
  username: string
  nickname: string | null
  email: string | null
  phone: string | null
  avatar_url: string | null
  created_at: string
}

interface ApiConfig {
  provider: string
  api_key: string | null
  base_url: string | null
  model: string | null
  max_tokens: number | null
  temperature: number | null
  retry_count: number | null
  response_language: string | null
}

const loading = ref(false)
const apiLoading = ref(false)
const saving = ref(false)
const activeTab = ref<'info' | 'password' | 'api'>('info')

const profile = ref<UserProfile>({
  id: 0,
  username: '',
  nickname: '',
  email: '',
  phone: '',
  avatar_url: '',
  created_at: ''
})

const editProfile = ref({
  nickname: '',
  email: '',
  phone: '',
  avatar_url: ''
})

const passwordForm = ref({
  old_password: '',
  new_password: '',
  confirm_password: ''
})

const apiConfig = ref<ApiConfig>({
  provider: 'anthropic',
  api_key: '',
  base_url: '',
  model: '',
  max_tokens: 4096,
  temperature: 0.7,
  retry_count: 3,
  response_language: ''
})

const apiMessage = ref({ type: '', text: '' })
const message = ref({ type: '', text: '' })
const passwordMessage = ref({ type: '', text: '' })

const isModified = computed(() => {
  return editProfile.value.nickname !== (profile.value.nickname || '') ||
    editProfile.value.email !== (profile.value.email || '') ||
    editProfile.value.phone !== (profile.value.phone || '') ||
    editProfile.value.avatar_url !== (profile.value.avatar_url || '')
})

// Track original API config for modification detection
const originalApiConfig = ref<ApiConfig | null>(null)

const isApiModified = computed(() => {
  if (!originalApiConfig.value) return false
  return apiConfig.value.provider !== originalApiConfig.value.provider ||
    apiConfig.value.api_key !== originalApiConfig.value.api_key ||
    apiConfig.value.base_url !== originalApiConfig.value.base_url ||
    apiConfig.value.model !== originalApiConfig.value.model ||
    apiConfig.value.max_tokens !== originalApiConfig.value.max_tokens ||
    apiConfig.value.temperature !== originalApiConfig.value.temperature ||
    apiConfig.value.retry_count !== originalApiConfig.value.retry_count ||
    apiConfig.value.response_language !== originalApiConfig.value.response_language
})

onMounted(async () => {
  await initLogger()
  info('Profile page loaded')
  await loadProfile()
  await loadApiConfig()
})

async function loadProfile() {
  const userStr = localStorage.getItem('user')
  if (!userStr) {
    warn('Profile load failed: no user session')
    return
  }

  const user = JSON.parse(userStr)
  loading.value = true

  try {
    profile.value = await invoke<UserProfile>('get_user_profile', {
      username: user.username
    })

    editProfile.value = {
      nickname: profile.value.nickname || '',
      email: profile.value.email || '',
      phone: profile.value.phone || '',
      avatar_url: profile.value.avatar_url || ''
    }
  } catch (e) {
    warn('Profile load failed', { error: e })
  } finally {
    loading.value = false
  }
}

async function loadApiConfig() {
  apiLoading.value = true
  try {
    const config = await invoke<ApiConfig>('harness_get_api_config')
    apiConfig.value = {
      provider: config.provider,
      api_key: config.api_key || '',
      base_url: config.base_url || '',
      model: config.model || '',
      max_tokens: config.max_tokens || 4096,
      temperature: config.temperature || 0.7,
      retry_count: config.retry_count ?? 3,
      response_language: config.response_language || ''
    }
    // Store original for modification detection
    originalApiConfig.value = { ...apiConfig.value }
  } catch (e) {
    warn('API config load failed', { error: e })
    showApiMessage('error', '加载配置失败')
  } finally {
    apiLoading.value = false
  }
}

function showMessage(type: string, text: string) {
  message.value = { type, text }
  setTimeout(() => { message.value = { type: '', text: '' } }, 3000)
}

function showPasswordMessage(type: string, text: string) {
  passwordMessage.value = { type, text }
  setTimeout(() => { passwordMessage.value = { type: '', text: '' } }, 3000)
}

function showApiMessage(type: string, text: string) {
  apiMessage.value = { type, text }
  setTimeout(() => { apiMessage.value = { type: '', text: '' } }, 3000)
}

async function saveProfile() {
  saving.value = true
  message.value = { type: '', text: '' }

  try {
    const updated = await invoke<UserProfile>('update_user_profile', {
      username: profile.value.username,
      request: {
        nickname: editProfile.value.nickname || null,
        email: editProfile.value.email || null,
        phone: editProfile.value.phone || null,
        avatar_url: editProfile.value.avatar_url || null
      }
    })

    profile.value = updated
    localStorage.setItem('user', JSON.stringify({
      id: updated.id,
      username: updated.username,
      nickname: updated.nickname
    }))

    showMessage('success', '保存成功')
  } catch (e) {
    showMessage('error', '保存失败: ' + e)
  } finally {
    saving.value = false
  }
}

async function changePassword() {
  passwordMessage.value = { type: '', text: '' }

  if (passwordForm.value.new_password !== passwordForm.value.confirm_password) {
    showPasswordMessage('error', '两次输入的新密码不一致')
    return
  }

  if (passwordForm.value.new_password.length < 6) {
    showPasswordMessage('error', '新密码长度至少为 6 位')
    return
  }

  saving.value = true

  try {
    await invoke('change_password', {
      username: profile.value.username,
      request: {
        old_password: passwordForm.value.old_password,
        new_password: passwordForm.value.new_password
      }
    })

    passwordForm.value = { old_password: '', new_password: '', confirm_password: '' }
    showPasswordMessage('success', '密码修改成功')
  } catch (e) {
    showPasswordMessage('error', '修改失败: ' + e)
  } finally {
    saving.value = false
  }
}

async function saveApiConfig() {
  saving.value = true
  apiMessage.value = { type: '', text: '' }

  try {
    await invoke('harness_save_api_config', {
      config: {
        provider: apiConfig.value.provider,
        api_key: apiConfig.value.api_key || null,
        base_url: apiConfig.value.base_url || null,
        model: apiConfig.value.model || null,
        max_tokens: apiConfig.value.max_tokens || null,
        temperature: apiConfig.value.temperature || null,
        retry_count: apiConfig.value.retry_count ?? null,
        response_language: apiConfig.value.response_language || null
      }
    })
    showApiMessage('success', 'API 配置已保存')
  } catch (e) {
    showApiMessage('error', '保存失败: ' + e)
  } finally {
    saving.value = false
  }
}

function formatDate(dateStr: string): string {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric'
  })
}
</script>

<template>
  <div class="profile-container">
    <header class="header">
      <h1>设置</h1>
    </header>

    <div v-if="loading" class="loading">
      <div class="spinner"></div>
    </div>

    <main v-else class="content">
      <div class="profile-card">
        <div class="tabs">
          <button
            :class="['tab', { active: activeTab === 'info' }]"
            @click="activeTab = 'info'"
          >
            个人信息
          </button>
          <button
            :class="['tab', { active: activeTab === 'password' }]"
            @click="activeTab = 'password'"
          >
            修改密码
          </button>
          <button
            :class="['tab', { active: activeTab === 'api' }]"
            @click="activeTab = 'api'"
          >
            API 配置
          </button>
        </div>

        <!-- Profile Info Tab -->
        <div v-if="activeTab === 'info'" class="tab-content">
          <div v-if="message.text" :class="['message', message.type]">
            {{ message.text }}
          </div>

          <div class="avatar-section">
            <div class="avatar">
              <img v-if="editProfile.avatar_url" :src="editProfile.avatar_url" alt="avatar" />
              <span v-else>{{ profile.username?.charAt(0).toUpperCase() }}</span>
            </div>
            <div class="user-info">
              <span class="username">{{ profile.username }}</span>
              <span class="join-date">加入于 {{ formatDate(profile.created_at) }}</span>
            </div>
          </div>

          <form @submit.prevent="saveProfile">
            <div class="form-group">
              <label for="nickname">昵称</label>
              <input
                id="nickname"
                v-model="editProfile.nickname"
                type="text"
                placeholder="设置您的昵称"
              />
            </div>

            <div class="form-group">
              <label for="email">邮箱</label>
              <input
                id="email"
                v-model="editProfile.email"
                type="email"
                placeholder="绑定邮箱"
              />
            </div>

            <div class="form-group">
              <label for="phone">手机号</label>
              <input
                id="phone"
                v-model="editProfile.phone"
                type="tel"
                placeholder="绑定手机号"
              />
            </div>

            <div class="form-group">
              <label for="avatar">头像 URL</label>
              <input
                id="avatar"
                v-model="editProfile.avatar_url"
                type="url"
                placeholder="输入头像图片链接"
              />
            </div>

            <div class="form-actions">
              <button type="submit" class="save-btn" :disabled="!isModified || saving">
                {{ saving ? '保存中...' : '保存修改' }}
              </button>
            </div>
          </form>
        </div>

        <!-- Password Tab -->
        <div v-if="activeTab === 'password'" class="tab-content">
          <div v-if="passwordMessage.text" :class="['message', passwordMessage.type]">
            {{ passwordMessage.text }}
          </div>

          <form @submit.prevent="changePassword">
            <div class="form-group">
              <label for="oldPassword">当前密码</label>
              <input
                id="oldPassword"
                v-model="passwordForm.old_password"
                type="password"
                placeholder="请输入当前密码"
                required
              />
            </div>

            <div class="form-group">
              <label for="newPassword">新密码</label>
              <input
                id="newPassword"
                v-model="passwordForm.new_password"
                type="password"
                placeholder="请输入新密码（至少6位）"
                required
              />
            </div>

            <div class="form-group">
              <label for="confirmPassword">确认新密码</label>
              <input
                id="confirmPassword"
                v-model="passwordForm.confirm_password"
                type="password"
                placeholder="请再次输入新密码"
                required
              />
            </div>

            <div class="form-actions">
              <button type="submit" class="save-btn" :disabled="saving">
                {{ saving ? '修改中...' : '修改密码' }}
              </button>
            </div>
          </form>
        </div>

        <!-- API Config Tab -->
        <div v-if="activeTab === 'api'" class="tab-content">
          <div v-if="apiMessage.text" :class="['message', apiMessage.type]">
            {{ apiMessage.text }}
          </div>

          <div v-if="apiLoading" class="api-loading">
            <div class="spinner-small"></div>
            <span>加载配置...</span>
          </div>

          <template v-else>
            <div class="api-provider-badge">
              {{ apiConfig.provider === 'anthropic' ? '🤖 Anthropic' : apiConfig.provider === 'openai' ? '🔴 OpenAI' : '⚡ XAI' }}
            </div>

            <form @submit.prevent="saveApiConfig">
            <div class="form-group">
              <label for="provider">Provider</label>
              <select id="provider" v-model="apiConfig.provider">
                <option value="anthropic">Anthropic</option>
                <option value="openai">OpenAI</option>
                <option value="xai">xAI</option>
              </select>
            </div>

            <div class="form-group">
              <label for="apiKey">API Key</label>
              <input
                id="apiKey"
                v-model="apiConfig.api_key"
                type="password"
                placeholder="输入 API Key"
              />
            </div>

            <div class="form-group">
              <label for="baseUrl">Base URL <span class="optional">(可选)</span></label>
              <input
                id="baseUrl"
                v-model="apiConfig.base_url"
                type="text"
                placeholder="如: https://api.anthropic.com"
              />
            </div>

            <div class="form-group">
              <label for="model">Model <span class="optional">(可选)</span></label>
              <input
                id="model"
                v-model="apiConfig.model"
                type="text"
                placeholder="如: claude-sonnet-4-20250514"
              />
            </div>

            <div class="form-row">
              <div class="form-group">
                <label for="maxTokens">Max Tokens <span class="optional">(可选)</span></label>
                <input
                  id="maxTokens"
                  v-model.number="apiConfig.max_tokens"
                  type="number"
                  placeholder="4096"
                />
              </div>

              <div class="form-group">
                <label for="temperature">Temperature <span class="optional">(可选)</span></label>
                <input
                  id="temperature"
                  v-model.number="apiConfig.temperature"
                  type="number"
                  step="0.1"
                  min="0"
                  max="2"
                  placeholder="0.7"
                />
              </div>
            </div>

            <div class="form-row">
              <div class="form-group">
                <label for="retryCount">重试次数 <span class="optional">(可选)</span></label>
                <input
                  id="retryCount"
                  v-model.number="apiConfig.retry_count"
                  type="number"
                  min="0"
                  max="10"
                  placeholder="3"
                />
              </div>

              <div class="form-group">
                <label for="responseLanguage">回复语言 <span class="optional">(可选)</span></label>
                <input
                  id="responseLanguage"
                  v-model="apiConfig.response_language"
                  type="text"
                  placeholder="如: 中文、English"
                />
              </div>
            </div>

            <div class="form-actions">
              <button type="submit" class="save-btn primary" :disabled="saving || !isApiModified">
                {{ saving ? '保存中...' : '保存配置' }}
              </button>
            </div>
          </form>
          </template>
        </div>
      </div>
    </main>
  </div>
</template>

<style scoped>
.profile-container {
  min-height: 100%;
  background: #0f0f1a;
}

.header {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px 24px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.header h1 {
  color: #fff;
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.loading {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 60vh;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-top-color: #667eea;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.api-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px;
  color: rgba(255, 255, 255, 0.5);
}

.spinner-small {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255, 255, 255, 0.1);
  border-top-color: #667eea;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.content {
  padding: 24px;
  max-width: 560px;
  margin: 0 auto;
}

.profile-card {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  overflow: hidden;
}

.tabs {
  display: flex;
  background: rgba(0, 0, 0, 0.2);
}

.tab {
  flex: 1;
  padding: 14px;
  background: none;
  border: none;
  font-size: 14px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
  transition: all 0.2s;
  border-bottom: 2px solid transparent;
}

.tab:hover {
  color: rgba(255, 255, 255, 0.8);
}

.tab.active {
  color: #667eea;
  border-bottom-color: #667eea;
}

.tab-content {
  padding: 24px;
}

.message {
  padding: 12px 16px;
  border-radius: 8px;
  margin-bottom: 20px;
  font-size: 14px;
}

.message.success {
  background: rgba(76, 175, 80, 0.2);
  color: #81c784;
  border: 1px solid rgba(76, 175, 80, 0.3);
}

.message.error {
  background: rgba(244, 67, 54, 0.2);
  color: #e57373;
  border: 1px solid rgba(244, 67, 54, 0.3);
}

.avatar-section {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 24px;
  padding-bottom: 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.avatar {
  width: 60px;
  height: 60px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: bold;
  color: #fff;
  overflow: hidden;
}

.avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.user-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.username {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
}

.join-date {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
}

.form-group {
  margin-bottom: 20px;
}

.form-row {
  display: flex;
  gap: 16px;
}

.form-row .form-group {
  flex: 1;
}

label {
  display: block;
  margin-bottom: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  font-weight: 500;
}

.optional {
  color: rgba(255, 255, 255, 0.4);
  font-weight: 400;
  font-size: 12px;
}

input, select {
  width: 100%;
  padding: 12px 14px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 14px;
  transition: all 0.2s;
  box-sizing: border-box;
}

input::placeholder {
  color: rgba(255, 255, 255, 0.3);
}

input:focus, select:focus {
  outline: none;
  border-color: #667eea;
  background: rgba(255, 255, 255, 0.1);
}

select {
  cursor: pointer;
}

select option {
  background: #1a1a2e;
  color: #fff;
}

.form-actions {
  margin-top: 24px;
}

.save-btn {
  width: 100%;
  padding: 14px;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.save-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-btn.primary {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border: none;
  color: #fff;
}

.save-btn.primary:hover:not(:disabled) {
  opacity: 0.9;
}

.api-provider-badge {
  display: inline-block;
  padding: 6px 12px;
  background: rgba(102, 126, 234, 0.2);
  border: 1px solid rgba(102, 126, 234, 0.3);
  border-radius: 16px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.8);
  margin-bottom: 24px;
}
</style>
