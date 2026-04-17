import { info as tauriInfo, warn as tauriWarn, error as tauriError, attachConsole } from '@tauri-apps/plugin-log'
import { ref } from 'vue'

export type LogLevel = 'info' | 'warn' | 'error' | 'debug'

const isInitialized = ref(false)

export interface LogEntry {
  timestamp: string
  level: LogLevel
  message: string
  data?: any
}

const logs = ref<LogEntry[]>([])

export function useLogger() {
  async function initLogger() {
    if (isInitialized.value) return

    try {
      await attachConsole()
      isInitialized.value = true
      await tauriInfo('[WEB] Logger initialized')
    } catch (e) {
      console.warn('Failed to attach Tauri logger:', e)
      isInitialized.value = true
    }
  }

  function formatMessage(message: string, data?: any): string {
    if (data !== undefined) {
      try {
        return `${message} | ${JSON.stringify(data)}`
      } catch {
        return message
      }
    }
    return message
  }

  async function log(level: LogLevel, message: string, data?: any) {
    const timestamp = new Date().toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false
    })

    const entry: LogEntry = {
      timestamp,
      level,
      message,
      data
    }

    logs.value.push(entry)

    if (logs.value.length > 100) {
      logs.value.shift()
    }

    const formatted = formatMessage(message, data)
    const webMessage = `[WEB] ${formatted}`

    try {
      switch (level) {
        case 'info':
          await tauriInfo(webMessage)
          break
        case 'warn':
          await tauriWarn(webMessage)
          break
        case 'error':
          await tauriError(webMessage)
          break
        case 'debug':
          if (import.meta.env.DEV) {
            console.debug(`[DEBUG] ${message}`, data)
          }
          break
      }
    } catch {
      // fallback to browser console if Tauri log fails
      const consoleMethod = level === 'error' ? 'error' : level === 'warn' ? 'warn' : 'log'
      console[consoleMethod](`[${level.toUpperCase()}] ${message}`, data)
    }
  }

  async function info(message: string, data?: any) {
    await log('info', message, data)
  }

  async function warn(message: string, data?: any) {
    await log('warn', message, data)
  }

  async function error(message: string, data?: any) {
    await log('error', message, data)
  }

  async function debug(message: string, data?: any) {
    await log('debug', message, data)
  }

  function getLogs(): LogEntry[] {
    return [...logs.value]
  }

  function clearLogs() {
    logs.value = []
  }

  return {
    initLogger,
    info,
    warn,
    error,
    debug,
    getLogs,
    clearLogs,
    logs
  }
}
