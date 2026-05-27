import { NavLink, Outlet } from "react-router-dom"
import { cn } from "@/lib/utils"

const navItems = [
  { to: "/", label: "控制面板" },
  { to: "/streams", label: "流管理" },
  { to: "/tasks", label: "任务管理" },
]

export function Layout() {
  return (
    <div className="min-h-screen bg-gray-50">
      <nav className="border-b bg-white px-6 py-3 flex items-center gap-6 shadow-sm">
        <span className="font-bold text-lg text-brand">GetFrame</span>
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              cn(
                "text-sm font-medium transition-colors",
                isActive ? "text-brand border-b-2 border-brand pb-1" : "text-gray-600 hover:text-gray-900",
              )
            }
          >
            {item.label}
          </NavLink>
        ))}
      </nav>
      <main className="p-6 max-w-7xl mx-auto">
        <Outlet />
      </main>
    </div>
  )
}
