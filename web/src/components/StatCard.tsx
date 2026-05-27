import { type LucideIcon } from "lucide-react"

interface StatCardProps {
  title: string
  value: string | number
  icon: LucideIcon
  color?: "success" | "brand" | "error" | "default"
}

const colorMap = {
  success: "text-success",
  brand: "text-brand",
  error: "text-error",
  default: "text-gray-900",
}

export function StatCard({ title, value, icon: Icon, color = "default" }: StatCardProps) {
  return (
    <div className="bg-white border rounded-xl p-5 shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-gray-500 uppercase tracking-wide">{title}</span>
        <Icon className={`w-5 h-5 ${colorMap[color]}`} />
      </div>
      <div className={`text-2xl font-bold ${colorMap[color]}`}>{value}</div>
    </div>
  )
}
