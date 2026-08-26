{{- define "vbmc-rs.namespace" -}}
{{ .Values.namespace }}
{{- end -}}

{{- define "vbmc-rs.fullImage" -}}
{{ .Values.image.registry }}/{{ .repository }}:{{ .tag }}
{{- end -}}

{{- define "vbmc-rs.aggregatorImage" -}}
{{ .Values.image.registry }}/{{ .Values.aggregator.image.repository }}:{{ .Values.aggregator.image.tag }}
{{- end -}}

{{- define "vbmc-rs.webhookImage" -}}
{{ .Values.image.registry }}/{{ .Values.webhook.image.repository }}:{{ .Values.webhook.image.tag }}
{{- end -}}

{{- define "vbmc-rs.sidecarImage" -}}
{{ .Values.webhook.sidecarImage }}
{{- end -}}
