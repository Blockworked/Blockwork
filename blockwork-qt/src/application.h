#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QVector>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlEngine>
#include <memory>

std::unique_ptr<QGuiApplication> blockwork_new_application(const QVector<QByteArray> &args);

void blockwork_init_appearance(QQmlEngine &engine, const QString &scheme);
