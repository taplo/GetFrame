import { useState } from "react"

interface FramePreviewProps {
  streamId: string
  latestFrameKey?: string | null
  refreshToken?: number
  className?: string
}

export function FramePreview({ streamId, latestFrameKey, refreshToken = 0, className = "" }: FramePreviewProps) {
  const [errored, setErrored] = useState(false)
  const url = latestFrameKey
    ? `/api/v1/streams/${streamId}/frames/latest?_=${refreshToken}`
    : null

  if (!url || errored) {
    return (
      <div className={`bg-gray-100 rounded-lg flex items-center justify-center text-gray-400 text-xs ${className}`}>
        暂无帧
      </div>
    )
  }

  return (
    <img
      src={url}
      alt="Latest frame"
      className={`object-cover rounded-lg ${className}`}
      onError={() => setErrored(true)}
    />
  )
}
