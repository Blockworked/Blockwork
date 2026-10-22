#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QVector>
#include <QtGui/QGuiApplication>
#include <memory>

std::unique_ptr<QGuiApplication> blockwork_new_application(const QVector<QByteArray> &args);
