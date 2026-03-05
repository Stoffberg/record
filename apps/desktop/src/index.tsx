/* @refresh reload */
import { render } from 'solid-js/web'
import './index.css'
import App from './App'
import { AppProvider } from './lib/context'

const root = document.getElementById('root')

render(
  () => (
    <AppProvider>
      <App />
    </AppProvider>
  ),
  root!,
)
