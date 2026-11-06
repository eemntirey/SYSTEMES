with open('src/components/taskbar/Taskbar.tsx') as f:
    content = f.read()
old = "            onMouseLeave={() => setHoveredAppId(null)}\n          />\n        )\n      )}"
new = "            onMouseLeave={() => setHoveredAppId(null)}\n            onClick={() => { if (openWin) toggleWindowMinimize(openWin.id); else openWindow(appId) }}\n          />\n        </div>\n      ))}"
content = content.replace(old, new)
with open('src/components/taskbar/Taskbar.tsx', 'w') as f:
    f.write(content)
print('fixed')
