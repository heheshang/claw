import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const token = ref<string | null>(localStorage.getItem('token'))
const user = ref<{ id: number; username: string } | null>(
  JSON.parse(localStorage.getItem('user') || 'null')
)

export function useAuth() {
  const isAuthenticated = computed(() => !!token.value)

  async function verifyToken(): Promise<boolean> {
    if (!token.value) return false

    try {
      await invoke('verify_token', { token: token.value })
      return true
    } catch {
      logout()
      return false
    }
  }

  function logout() {
    token.value = null
    user.value = null
    localStorage.removeItem('token')
    localStorage.removeItem('user')
  }

  function setAuth(newToken: string, newUser: { id: number; username: string }) {
    token.value = newToken
    user.value = newUser
    localStorage.setItem('token', newToken)
    localStorage.setItem('user', JSON.stringify(newUser))
  }

  return {
    token,
    user,
    isAuthenticated,
    verifyToken,
    logout,
    setAuth
  }
}
