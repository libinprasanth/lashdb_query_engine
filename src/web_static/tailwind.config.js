/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,jsx,ts,tsx}', "*"],
  theme: {
    extend: {
      colors: {
        primary: '#0CBCC5',
        primaryLight: '#E8F9FA',
        primaryDark: '#0A9AA0',
        background: '#FAF7F2',
        backgroundDark: '#0CBCC5',
        text: '#1a2535',
        textSecondary: '#9aa3ad',
        textMuted: '#b0b8c4',
        error: '#c05050',
        errorLight: '#FFF0F0',
        errorBackground: '#FFF0EB',
        divider: '#F0EDE8',
        cardBackground: '#F5F3EF',
      },
    },
  },
  plugins: [],
}