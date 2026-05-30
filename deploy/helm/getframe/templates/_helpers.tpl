{{/*
GetFrame labels
*/}}
{{- define "getframe.labels" -}}
app.kubernetes.io/name: getframe
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion }}
app.kubernetes.io/component: worker
app.kubernetes.io/part-of: getframe
{{- end -}}

{{/*
Selector labels
*/}}
{{- define "getframe.selectorLabels" -}}
app.kubernetes.io/name: getframe
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: worker
{{- end -}}
