import { mount } from 'svelte'
import './app.css'
import './lib/pixel-icons.css'
import App from './App.svelte'
import { initPWA } from './lib/pwa'

// iOS Safari applies :active (press feedback) only once a touch listener exists.
document.addEventListener('touchstart', () => {}, { passive: true })

// Initialize PWA service worker
initPWA()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
