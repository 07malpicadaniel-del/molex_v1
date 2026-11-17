/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        molex: {
          bg: "#121212",
          surface: "#1E1E1E",
          variant: "#2C2C2C",
          accent: "#8D6E63",
          dark: "#5D4037",
          light: "#D7CCC8",
          text: "#F5F5F5",
          muted: "#A0A0A0",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
        mono: ["JetBrains Mono", "Fira Code", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
